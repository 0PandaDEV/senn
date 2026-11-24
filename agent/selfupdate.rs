use std::{os::unix::fs::PermissionsExt, path::Path, time::Duration};

use sha2::{Digest, Sha256};

fn repo() -> String {
    std::env::var("SENN_REPO").unwrap_or_else(|_| "0PandaDEV/senn".into())
}

pub async fn run(tag: &str, line: impl Fn(String)) -> Result<(), String> {
    if Path::new("/.dockerenv").exists() {
        return Err("this agent runs in a container, update its image instead".into());
    }
    if tag.is_empty() || tag.contains(['/', ' ', '?', '#']) {
        return Err(format!("invalid release tag {tag}"));
    }
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => return Err(format!("unsupported architecture {other}")),
    };
    let current = env!("CARGO_PKG_VERSION");
    if tag.strip_prefix('v').unwrap_or(tag) == current {
        line(format!("» already on {tag}"));
        return Ok(());
    }
    let name = format!("senn-agent-linux-{arch}");
    let base = format!("https://github.com/{}/releases/download/{tag}", repo());
    let client = reqwest::Client::builder()
        .user_agent("senn-agent")
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let get = |url: String| {
        let client = client.clone();
        async move {
            client
                .get(&url)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("download {url}: {e}"))?
                .bytes()
                .await
                .map_err(|e| format!("download {url}: {e}"))
        }
    };
    line(format!("updating agent {current} → {tag}"));
    let sums = get(format!("{base}/checksums.txt")).await?;
    let expected = String::from_utf8_lossy(&sums)
        .lines()
        .find_map(|l| {
            let mut parts = l.split_whitespace();
            let hash = parts.next()?;
            (parts.next()? == name).then(|| hash.to_ascii_lowercase())
        })
        .ok_or_else(|| format!("no checksum for {name} in {tag}"))?;
    line(format!("downloading {name}"));
    let binary = get(format!("{base}/{name}")).await?;
    let actual: String = Sha256::digest(&binary)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != expected {
        return Err("checksum mismatch, nothing was changed".into());
    }
    line(format!(
        "checksum ok ({} MiB)",
        binary.len() / (1024 * 1024)
    ));
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe
        .to_str()
        .and_then(|s| s.strip_suffix(" (deleted)"))
        .map(std::path::PathBuf::from)
        .unwrap_or(exe);
    let staged = exe.with_file_name(".senn-agent.new");
    tokio::fs::write(&staged, &binary)
        .await
        .map_err(|e| format!("write {}: {e}", staged.display()))?;
    tokio::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = tokio::fs::rename(&staged, &exe).await {
        let _ = tokio::fs::remove_file(&staged).await;
        return Err(format!("replace {}: {e}", exe.display()));
    }
    line(format!("» installed {tag}, restarting agent"));
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        std::process::exit(0);
    });
    Ok(())
}
