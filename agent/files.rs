use std::path::{Component, Path, PathBuf};

use serde::Serialize;

pub const MAX_FILE: u64 = 2 * 1024 * 1024;

#[derive(Serialize)]
pub struct Entry {
    pub name: String,
    pub kind: &'static str,
    pub size: u64,
    pub modified: i64,
}

pub fn root(stacks_dir: &str) -> Result<PathBuf, String> {
    std::fs::canonicalize(stacks_dir).map_err(|e| format!("stacks dir {stacks_dir}: {e}"))
}

fn relative(path: &str) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for component in Path::new(path.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err("path escapes the stacks directory".into()),
        }
    }
    Ok(out)
}

pub fn resolve(stacks_dir: &str, path: &str) -> Result<PathBuf, String> {
    let root = root(stacks_dir)?;
    let joined = root.join(relative(path)?);
    let checked = match std::fs::canonicalize(&joined) {
        Ok(real) => real,
        Err(_) => {
            let parent = joined.parent().ok_or("invalid path")?;
            let real_parent =
                std::fs::canonicalize(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            real_parent.join(joined.file_name().ok_or("invalid path")?)
        }
    };
    if checked != root && !checked.starts_with(&root) {
        return Err("path escapes the stacks directory".into());
    }
    Ok(checked)
}

pub fn resolve_entry(stacks_dir: &str, path: &str) -> Result<PathBuf, String> {
    let root = root(stacks_dir)?;
    let rel = relative(path)?;
    let name = rel.file_name().ok_or("invalid path")?.to_os_string();
    let parent = root.join(rel.parent().unwrap_or(Path::new("")));
    let real_parent =
        std::fs::canonicalize(&parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    if real_parent != root && !real_parent.starts_with(&root) {
        return Err("path escapes the stacks directory".into());
    }
    Ok(real_parent.join(name))
}

pub fn modified(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn list(stacks_dir: &str, path: &str) -> Result<Vec<Entry>, String> {
    let dir = resolve(stacks_dir, path)?;
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let link = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        let kind = if link {
            "symlink"
        } else if meta.is_dir() {
            "dir"
        } else {
            "file"
        };
        entries.push(Entry {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind,
            size: meta.len(),
            modified: modified(&meta),
        });
    }
    entries.sort_by(|a, b| {
        (a.kind != "dir", a.name.to_lowercase()).cmp(&(b.kind != "dir", b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn read(stacks_dir: &str, path: &str) -> Result<String, String> {
    let file = resolve(stacks_dir, path)?;
    let meta = std::fs::metadata(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    if meta.len() > MAX_FILE {
        return Err(format!(
            "file is larger than {} MiB",
            MAX_FILE / 1024 / 1024
        ));
    }
    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|_| "file is not valid UTF-8 text".into())
}

#[derive(Serialize)]
pub struct TreeFile {
    pub path: String,
    pub content: String,
}

const TREE_SKIP: [&str; 4] = [".git", "node_modules", ".venv", "__pycache__"];
const TREE_FILE_MAX: u64 = 256 * 1024;
const TREE_TOTAL_MAX: usize = 4 * 1024 * 1024;
const TREE_COUNT_MAX: usize = 500;

pub fn read_tree(stacks_dir: &str, path: &str) -> Result<Vec<TreeFile>, String> {
    let base = resolve(stacks_dir, path)?;
    let mut out = Vec::new();
    let mut total = 0usize;
    let mut stack = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = read.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let full = entry.path();
            if kind.is_dir() {
                if !TREE_SKIP.contains(&name.as_str()) {
                    stack.push(full);
                }
                continue;
            }
            if !kind.is_file() || out.len() >= TREE_COUNT_MAX {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.len() > TREE_FILE_MAX || total + meta.len() as usize > TREE_TOTAL_MAX {
                continue;
            }
            let Ok(bytes) = std::fs::read(&full) else {
                continue;
            };
            if bytes.contains(&0) {
                continue;
            }
            let Ok(content) = String::from_utf8(bytes) else {
                continue;
            };
            total += content.len();
            let rel = full
                .strip_prefix(&base)
                .unwrap_or(&full)
                .to_string_lossy()
                .into_owned();
            out.push(TreeFile { path: rel, content });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

pub fn write(stacks_dir: &str, path: &str, content: &str) -> Result<(), String> {
    use std::io::Write;
    let file = resolve(stacks_dir, path)?;
    if content.len() as u64 > MAX_FILE {
        return Err("content too large".into());
    }
    if file.is_dir() {
        return Err("path is a directory".into());
    }
    let existed = file.exists();
    let mut handle = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&file)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    handle
        .write_all(content.as_bytes())
        .map_err(|e| e.to_string())?;
    handle.sync_all().map_err(|e| e.to_string())?;
    if !existed {
        adopt_owner(&file);
    }
    Ok(())
}

fn adopt_owner(path: &Path) {
    use std::os::unix::fs::MetadataExt;
    let Some(parent) = path.parent() else { return };
    if let Ok(meta) = std::fs::metadata(parent) {
        let _ = std::os::unix::fs::chown(path, Some(meta.uid()), Some(meta.gid()));
    }
}

pub fn make_dir(stacks_dir: &str, path: &str) -> Result<(), String> {
    let dir = resolve(stacks_dir, path)?;
    let existed = dir.exists();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !existed {
        adopt_owner(&dir);
    }
    Ok(())
}

pub fn remove(stacks_dir: &str, path: &str) -> Result<(), String> {
    let root = root(stacks_dir)?;
    let target = resolve_entry(stacks_dir, path)?;
    if target == root {
        return Err("refusing to remove the stacks directory".into());
    }
    let meta =
        std::fs::symlink_metadata(&target).map_err(|e| format!("{}: {e}", target.display()))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(&target).map_err(|e| e.to_string())
    } else {
        std::fs::remove_file(&target).map_err(|e| e.to_string())
    }
}

pub fn rename(stacks_dir: &str, from: &str, to: &str) -> Result<(), String> {
    let root = root(stacks_dir)?;
    let a = resolve_entry(stacks_dir, from)?;
    let b = resolve_entry(stacks_dir, to)?;
    if a == root || b == root {
        return Err("refusing to move the stacks directory".into());
    }
    if b.exists() {
        return Err("destination exists".into());
    }
    std::fs::rename(&a, &b).map_err(|e| e.to_string())
}
