use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Local {
    pub url: String,
    pub token: String,
    #[serde(default = "default_stacks_dir")]
    pub stacks_dir: String,
}

pub fn default_stacks_dir() -> String {
    "/opt/stacks".into()
}

pub fn config_path(arg: Option<PathBuf>) -> PathBuf {
    arg.or_else(|| std::env::var_os("SENN_AGENT_CONFIG").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/etc/senn-agent/config.toml"))
}

pub fn state_dir(arg: Option<PathBuf>) -> PathBuf {
    arg.or_else(|| std::env::var_os("SENN_AGENT_STATE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/var/lib/senn-agent"))
}

pub fn load(path: &std::path::Path) -> Result<Local, String> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "read {}: {e} (run `senn-agent install --url ... --token ...` first)",
            path.display()
        )
    })?;
    toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}

pub fn save(path: &std::path::Path, local: &Local) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let text = toml::to_string(local).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
