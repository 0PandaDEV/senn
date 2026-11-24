use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Mutex,
    time::{Duration, SystemTime},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

use crate::proto::{StackAction, StackInfo};

pub const COMPOSE_FILES: [&str; 4] = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

pub fn is_compose_file(name: &str) -> bool {
    COMPOSE_FILES.contains(&name)
}

pub fn find_file(dir: &Path) -> Option<PathBuf> {
    COMPOSE_FILES
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

#[derive(Clone)]
struct Cached {
    stamp: Vec<Option<SystemTime>>,
    services: Vec<String>,
    error: Option<String>,
}

static CACHE: Mutex<Option<HashMap<PathBuf, Cached>>> = Mutex::new(None);

const WATCHED: [&str; 7] = [
    ".env",
    "compose.override.yaml",
    "compose.override.yml",
    "docker-compose.override.yaml",
    "docker-compose.override.yml",
    "compose.yaml",
    "docker-compose.yml",
];

fn stamp(file: &Path) -> Vec<Option<SystemTime>> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let mut out: Vec<Option<SystemTime>> = [Some(file.to_path_buf())]
        .into_iter()
        .chain(WATCHED.iter().map(|n| Some(dir.join(n))))
        .map(|p| {
            p.and_then(|p| std::fs::metadata(p).ok())
                .and_then(|m| m.modified().ok())
        })
        .collect();
    out.dedup();
    out
}

pub fn base_command(bin: &str, dir: &Path) -> Command {
    let mut cmd = Command::new(bin);
    cmd.arg("compose")
        .arg("--ansi")
        .arg("never")
        .env("COMPOSE_PROGRESS", "plain")
        .current_dir(dir)
        .stdin(Stdio::null());
    cmd
}

async fn services(bin: &str, dir: &Path) -> (Vec<String>, Option<String>) {
    let mut cmd = base_command(bin, dir);
    cmd.args(["config", "--services"]);
    let run = tokio::time::timeout(Duration::from_secs(15), cmd.output()).await;
    match run {
        Ok(Ok(out)) if out.status.success() => {
            let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            names.sort();
            (names, None)
        }
        Ok(Ok(out)) => (
            Vec::new(),
            Some(last_line(&String::from_utf8_lossy(&out.stderr))),
        ),
        Ok(Err(e)) => (Vec::new(), Some(e.to_string())),
        Err(_) => (Vec::new(), Some("compose config timed out".into())),
    }
}

fn last_line(text: &str) -> String {
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("compose config failed")
        .trim()
        .to_string()
}

pub async fn discover(bin: &str, root: &Path) -> Vec<StackInfo> {
    let Ok(read) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = read
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            find_file(&e.path()).map(|f| (name, f))
        })
        .collect();
    found.sort_by_key(|a| a.0.to_lowercase());
    let mut out = Vec::new();
    for (name, file) in found {
        let dir = file.parent().unwrap_or(root).to_path_buf();
        let now = stamp(&file);
        let cached = CACHE
            .lock()
            .ok()
            .and_then(|c| c.as_ref().and_then(|m| m.get(&file).cloned()))
            .filter(|c| c.stamp == now);
        let entry = match cached {
            Some(c) => c,
            None => {
                let (services, error) = services(bin, &dir).await;
                let c = Cached {
                    stamp: now,
                    services,
                    error,
                };
                if let Ok(mut cache) = CACHE.lock() {
                    cache
                        .get_or_insert_with(HashMap::new)
                        .insert(file.clone(), c.clone());
                }
                c
            }
        };
        out.push(StackInfo {
            dir: dir.to_string_lossy().into_owned(),
            file: file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            name,
            services: entry.services,
            error: entry.error,
        });
    }
    out
}

pub async fn validate(bin: &str, dir: &Path, content: &str) -> Result<(), String> {
    let mut cmd = Command::new(bin);
    cmd.arg("compose")
        .arg("--project-directory")
        .arg(dir)
        .args(["-f", "-", "config", "-q"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(content.as_bytes()).await;
    }
    let out = tokio::time::timeout(Duration::from_secs(20), child.wait_with_output())
        .await
        .map_err(|_| "validation timed out".to_string())?
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn steps(action: StackAction, service: Option<&str>) -> Vec<Vec<String>> {
    let svc = |mut args: Vec<String>| {
        if let Some(s) = service {
            args.push(s.to_string());
        }
        args
    };
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match action {
        StackAction::Up => vec![svc(v(&["up", "-d"]))],
        StackAction::Down => vec![v(&["down"])],
        StackAction::Start => vec![svc(v(&["start"]))],
        StackAction::Stop => vec![svc(v(&["stop"]))],
        StackAction::Restart => vec![svc(v(&["restart"]))],
        StackAction::Pull => vec![svc(v(&["pull"]))],
        StackAction::Update => {
            let mut args = v(&["up", "-d", "--force-recreate", "--pull", "always"]);
            if let Some(s) = service {
                args.push("--no-deps".into());
                args.push(s.to_string());
            }
            vec![args]
        }
        StackAction::Delete => vec![v(&["down", "--remove-orphans"])],
    }
}

pub async fn run(
    bin: &str,
    dir: &Path,
    args: &[String],
    mut line: impl FnMut(String),
) -> Result<(), String> {
    let mut cmd = base_command(bin, dir);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    line(format!("$ docker compose {}", args.join(" ")));
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    for stream in [
        Box::new(BufReader::new(stdout)) as Box<dyn tokio::io::AsyncBufRead + Unpin + Send>,
        Box::new(BufReader::new(stderr)),
    ] {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = stream.lines();
            while let Ok(Some(l)) = lines.next_line().await {
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);
    while let Some(l) = rx.recv().await {
        line(l);
    }
    let status = child.wait().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("exited with {status}"))
    }
}
