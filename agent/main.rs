#[path = "../shared/proto.rs"]
mod proto;

#[path = "../shared/cron.rs"]
mod cron;

mod compose;
mod config;
mod docker;
mod files;
mod handlers;
mod host;
mod link;
mod registry;
mod resources;
mod selfupdate;
mod updates;

use std::{path::PathBuf, sync::Arc};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "senn-agent", version, about = "Senn host agent")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    state_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run,
    Install {
        #[arg(long, requires = "token")]
        url: Option<String>,
        #[arg(long, requires = "url")]
        token: Option<String>,
        #[arg(long)]
        stacks_dir: Option<String>,
    },
    Uninstall,
}

const UNIT: &str = "/etc/systemd/system/senn-agent.service";

fn systemctl(args: &[&str]) -> Result<(), String> {
    let status = std::process::Command::new("systemctl")
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("systemctl {} failed", args.join(" ")))
    }
}

fn install(config: PathBuf, state: PathBuf) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = PathBuf::from("/usr/local/bin/senn-agent");
    if exe != target {
        let staged = PathBuf::from("/usr/local/bin/senn-agent.new");
        std::fs::copy(&exe, &staged)
            .map_err(|e| format!("copy to {}: {e} (run as root)", staged.display()))?;
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        std::fs::rename(&staged, &target)
            .map_err(|e| format!("replace {}: {e}", target.display()))?;
    }
    std::fs::create_dir_all(&state).map_err(|e| e.to_string())?;
    let unit = format!(
        "[Unit]\nDescription=Senn agent\nAfter=network-online.target docker.service\nWants=network-online.target\n\n[Service]\nExecStart={} run --config {} --state-dir {}\nRestart=always\nRestartSec=3\nEnvironment=HOME=/root\n\n[Install]\nWantedBy=multi-user.target\n",
        target.display(),
        config.display(),
        state.display()
    );
    std::fs::write(UNIT, unit).map_err(|e| format!("write {UNIT}: {e} (run as root)"))?;
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "senn-agent"])?;
    systemctl(&["restart", "senn-agent"])?;
    println!("senn-agent installed and running the new binary");
    Ok(())
}

fn configure(
    path: &std::path::Path,
    url: Option<String>,
    token: Option<String>,
    stacks_dir: Option<String>,
) -> Result<(), String> {
    let existing = config::load(path).ok();
    let (url, token) = match (url, token, &existing) {
        (Some(url), Some(token), _) => (url, token),
        (_, _, Some(local)) => (local.url.clone(), local.token.clone()),
        _ => return Err("no config yet, pass --url and --token".into()),
    };
    let stacks_dir = stacks_dir
        .or_else(|| existing.map(|l| l.stacks_dir))
        .unwrap_or_else(config::default_stacks_dir);
    if !stacks_dir.starts_with('/') {
        return Err("--stacks-dir must be an absolute path".into());
    }
    let stacks_dir = if stacks_dir.len() > 1 {
        stacks_dir.trim_end_matches('/').to_string()
    } else {
        stacks_dir
    };
    config::save(
        path,
        &config::Local {
            url,
            token,
            stacks_dir,
        },
    )?;
    println!("saved {}", path.display());
    Ok(())
}

fn uninstall() -> Result<(), String> {
    let _ = systemctl(&["disable", "--now", "senn-agent"]);
    let _ = std::fs::remove_file(UNIT);
    let _ = std::fs::remove_file("/usr/local/bin/senn-agent");
    systemctl(&["daemon-reload"])?;
    println!("senn-agent removed (config and state left in place)");
    Ok(())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config_path = config::config_path(cli.config);
    let state_dir = config::state_dir(cli.state_dir);
    let result: Result<(), String> = match cli.command {
        Command::Install {
            url,
            token,
            stacks_dir,
        } => configure(&config_path, url, token, stacks_dir)
            .and_then(|_| install(config_path, state_dir)),
        Command::Uninstall => uninstall(),
        Command::Run => run(config_path, state_dir).await,
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run(config_path: PathBuf, state_dir: PathBuf) -> Result<(), String> {
    let local = config::load(&config_path)?;
    std::fs::create_dir_all(&state_dir).map_err(|e| format!("{}: {e}", state_dir.display()))?;
    let docker = docker::connect()?;
    let ctx = Arc::new(handlers::Ctx::new(
        state_dir,
        local.stacks_dir.clone(),
        docker,
    ));
    tokio::select! {
        _ = link::run(local, ctx) => {}
        _ = tokio::signal::ctrl_c() => {}
    }
    Ok(())
}
