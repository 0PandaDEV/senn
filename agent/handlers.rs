use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use bollard::Docker;
use futures::{StreamExt, stream::select_all};
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    compose, docker, files, resources,
    proto::{AgentConfig, FromAgent, HostState, Op, ResourceKind, StackAction},
    updates::Updates,
};

pub struct Ctx {
    pub docker: Docker,
    pub docker_bin: String,
    pub cfg: RwLock<AgentConfig>,
    pub updates: Updates,
    pub state_dir: PathBuf,
    pub stacks_dir: String,
    pub samples: RwLock<HashMap<String, docker::Sample>>,
    pub host: RwLock<Option<crate::proto::HostStats>>,
    busy: Mutex<HashSet<String>>,
}

impl Ctx {
    pub fn new(state_dir: PathBuf, stacks_dir: String, docker: Docker) -> Self {
        let cfg = std::fs::read_to_string(state_dir.join("config.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            docker,
            docker_bin: std::env::var("SENN_AGENT_DOCKER_BIN").unwrap_or_else(|_| "docker".into()),
            cfg: RwLock::new(cfg),
            updates: Updates::load(&state_dir),
            state_dir,
            stacks_dir,
            samples: RwLock::new(HashMap::new()),
            host: RwLock::new(None),
            busy: Mutex::new(HashSet::new()),
        }
    }

    pub fn set_config(&self, config: AgentConfig) {
        if let Ok(text) = serde_json::to_string(&config) {
            let _ = std::fs::write(self.state_dir.join("config.json"), text);
        }
        *self.cfg.write().expect("cfg lock") = config;
    }

    pub fn config(&self) -> AgentConfig {
        self.cfg.read().expect("cfg lock").clone()
    }

    fn lock(&self, key: &str) -> Option<BusyGuard<'_>> {
        let mut set = self.busy.lock().expect("busy lock");
        if set.insert(key.to_string()) {
            Some(BusyGuard {
                ctx: self,
                key: key.to_string(),
            })
        } else {
            None
        }
    }
}

struct BusyGuard<'a> {
    ctx: &'a Ctx,
    key: String,
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.ctx.busy.lock().expect("busy lock").remove(&self.key);
    }
}

pub async fn state(ctx: &Ctx) -> Result<HostState, String> {
    let root = files::root(&ctx.stacks_dir).ok();
    let stacks = match &root {
        Some(r) => compose::discover(&ctx.docker_bin, r).await,
        None => Vec::new(),
    };
    let mut dirs = HashMap::new();
    for s in &stacks {
        dirs.insert(
            format!("{}/{}", ctx.stacks_dir.trim_end_matches('/'), s.name),
            s.name.clone(),
        );
        if let Some(r) = &root {
            dirs.insert(
                r.join(&s.name).to_string_lossy().into_owned(),
                s.name.clone(),
            );
        }
    }
    let samples = ctx.samples.read().expect("samples lock").clone();
    let mut containers = docker::containers(&ctx.docker, &dirs, &samples).await?;
    let cfg = ctx.config();
    let fallback = cfg.policy_default();
    for c in &mut containers {
        let overridden = c.stack.as_deref().and_then(|stack| {
            c.service
                .as_deref()
                .and_then(|s| cfg.policies.get(&format!("{stack}/{s}")))
                .or_else(|| cfg.policies.get(stack))
        });
        c.update_policy = match overridden {
            Some(p) => p.clone(),
            None if c.update_policy.is_empty() => fallback.to_string(),
            None => c.update_policy.clone(),
        };
    }
    let (updates, updates_checked_at) = ctx.updates.snapshot();
    Ok(HostState {
        stacks,
        containers,
        updates,
        updates_checked_at,
        stacks_dir: ctx.stacks_dir.clone(),
        stacks_dir_ok: root.is_some(),
        host: ctx.host.read().expect("host lock").clone(),
    })
}

fn stack_paths(stacks_dir: &str, stack: &str) -> Result<(PathBuf, PathBuf), String> {
    if stack.is_empty() || stack.contains('/') || stack.starts_with('.') {
        return Err("invalid stack name".into());
    }
    let dir = files::resolve(stacks_dir, stack)?;
    let file = compose::find_file(&dir).ok_or_else(|| format!("no compose file in {stack}"))?;
    Ok((dir, file))
}

pub async fn run_stack(
    ctx: &Ctx,
    stack: &str,
    action: StackAction,
    service: Option<&str>,
    mut line: impl FnMut(String),
) -> Result<(), String> {
    let (dir, _) = stack_paths(&ctx.stacks_dir, stack)?;
    let _guard = ctx
        .lock(stack)
        .ok_or_else(|| format!("another operation on {stack} is still running"))?;
    for args in compose::steps(action, service) {
        compose::run(&ctx.docker_bin, &dir, &args, &mut line).await?;
    }
    if action == StackAction::Delete {
        files::remove(&ctx.stacks_dir, stack)?;
        line(format!("removed {}", dir.display()));
    }
    if action == StackAction::Update {
        if let Ok(st) = state(ctx).await {
            let images: Vec<String> = st
                .containers
                .iter()
                .filter(|c| c.stack.as_deref() == Some(stack))
                .filter(|c| service.is_none() || c.service.as_deref() == service)
                .map(|c| c.image.clone())
                .collect();
            ctx.updates.refresh(&ctx.docker, &st.containers, &images).await;
        }
    }
    Ok(())
}

pub async fn check_updates(ctx: &Ctx, progress: impl FnMut(String)) -> Result<(), String> {
    let st = state(ctx).await?;
    let watched: Vec<_> = st
        .containers
        .iter()
        .filter(|c| c.update_policy != "off")
        .cloned()
        .collect();
    ctx.updates.check(&ctx.docker, &watched, progress).await;
    Ok(())
}

pub async fn update_all(ctx: &Ctx, mut line: impl FnMut(String)) -> Result<(), String> {
    let st = state(ctx).await?;
    let behind: HashSet<&str> = st
        .updates
        .iter()
        .filter(|u| u.status == "available")
        .map(|u| u.image.as_str())
        .collect();
    let mut targets: Vec<(String, String)> = st
        .containers
        .iter()
        .filter(|c| c.update_policy != "off" && behind.contains(c.image.as_str()))
        .filter_map(|c| Some((c.stack.clone()?, c.service.clone()?)))
        .collect();
    targets.sort();
    targets.dedup();
    if targets.is_empty() {
        line("nothing to update".into());
        return Ok(());
    }
    let total = targets.len();
    let mut failed = Vec::new();
    for (i, (stack, service)) in targets.into_iter().enumerate() {
        line(format!("» [{}/{total}] {stack}/{service}", i + 1));
        if let Err(e) = run_stack(ctx, &stack, StackAction::Update, Some(&service), &mut line).await {
            line(format!("failed: {e}"));
            failed.push(format!("{stack}/{service}"));
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("{} of {total} failed: {}", failed.len(), failed.join(", ")))
    }
}

pub async fn auto_update(ctx: &Ctx, out: &UnboundedSender<FromAgent>) {
    let Ok(st) = state(ctx).await else { return };
    let behind: HashSet<&str> = st
        .updates
        .iter()
        .filter(|u| u.status == "available")
        .map(|u| u.image.as_str())
        .collect();
    let mut targets: Vec<(String, String)> = st
        .containers
        .iter()
        .filter(|c| c.update_policy == "auto" && behind.contains(c.image.as_str()))
        .filter_map(|c| Some((c.stack.clone()?, c.service.clone()?)))
        .collect();
    targets.sort();
    targets.dedup();
    for (stack, service) in targets {
        tracing::info!("auto update {stack}/{service}");
        let mut log = Vec::new();
        let result = run_stack(ctx, &stack, StackAction::Update, Some(&service), |l| {
            log.push(l)
        })
        .await;
        let _ = out.send(FromAgent::Event {
            kind: "auto_update".into(),
            data: json!({ "stack": stack, "service": service, "ok": result.is_ok(), "error": result.err(), "tail": log.iter().rev().take(5).rev().collect::<Vec<_>>() }),
        });
    }
}

pub fn spawn_stream(
    ctx: Arc<Ctx>,
    id: u64,
    op: Op,
    out: UnboundedSender<FromAgent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let chunk = |data: String| {
            let _ = out.send(FromAgent::Chunk { id, data });
        };
        let result: Result<(), String> = match op {
            Op::Stack {
                stack,
                action,
                service,
            } => run_stack(&ctx, &stack, action, service.as_deref(), chunk).await,
            Op::CheckUpdates => check_updates(&ctx, chunk).await,
            Op::UpdateAll => update_all(&ctx, chunk).await,
            Op::Prune { kind } => {
                let what = match kind {
                    ResourceKind::Image => "images",
                    ResourceKind::Volume => "volumes",
                    ResourceKind::Network => "networks",
                };
                chunk(format!("pruning unused {what}…"));
                resources::prune(&ctx.docker, kind).await.map(|(count, space)| {
                    if kind == ResourceKind::Network {
                        chunk(format!("» removed {count} {what}"));
                    } else {
                        chunk(format!("» freed {} · removed {count} {what}", resources::size(space)));
                    }
                })
            }
            Op::Logs {
                container,
                stack,
                containers,
                tail,
                follow,
            } => logs(&ctx, container, stack, containers, tail, follow, &out, id).await,
            _ => Err("not a streaming operation".into()),
        };
        let _ = out.send(FromAgent::End {
            id,
            ok: result.is_ok(),
            error: result.err(),
        });
    })
}

#[allow(clippy::too_many_arguments)]
async fn logs(
    ctx: &Ctx,
    container: Option<String>,
    stack: Option<String>,
    containers: Vec<String>,
    tail: u32,
    follow: bool,
    out: &UnboundedSender<FromAgent>,
    id: u64,
) -> Result<(), String> {
    let mut sources: Vec<(String, String)> = Vec::new();
    if !containers.is_empty() {
        let st = state(ctx).await?;
        for c in st.containers.into_iter().filter(|c| containers.contains(&c.id)) {
            let label = match (&c.stack, &c.service) {
                (Some(s), Some(v)) => format!("{s}/{v}"),
                _ => c.name.clone(),
            };
            sources.push((label, c.id));
        }
    } else if let Some(c) = container {
        sources.push((String::new(), c));
    } else if let Some(s) = stack {
        let st = state(ctx).await?;
        for c in st
            .containers
            .into_iter()
            .filter(|c| c.stack.as_deref() == Some(&s))
        {
            sources.push((c.service.unwrap_or(c.name), c.id));
        }
    }
    if sources.is_empty() {
        return Err("nothing to read logs from".into());
    }
    let prefix = sources.len() > 1;
    let streams = sources.into_iter().map(|(label, cid)| {
        docker::logs(&ctx.docker, &cid, tail, follow)
            .map(move |r| r.map(|l| (label.clone(), l)))
            .boxed()
    });
    let mut merged = select_all(streams);
    while let Some(item) = merged.next().await {
        let (label, text) = item?;
        for l in text.split('\n').filter(|l| !l.is_empty()) {
            let data = if prefix {
                format!("{label}\t{l}")
            } else {
                l.to_string()
            };
            if out.send(FromAgent::Chunk { id, data }).is_err() {
                return Ok(());
            }
        }
    }
    Ok(())
}

pub async fn handle(ctx: &Ctx, op: Op) -> Result<Value, String> {
    let dir = ctx.stacks_dir.clone();
    let blocking = |f: Box<dyn FnOnce() -> Result<Value, String> + Send>| async move {
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|e| e.to_string())?
    };
    match op {
        Op::State => serde_json::to_value(state(ctx).await?).map_err(|e| e.to_string()),
        Op::Container { id, action } => {
            docker::container_action(&ctx.docker, &id, action).await?;
            Ok(Value::Null)
        }
        Op::ListDir { path } => {
            blocking(Box::new(move || {
                serde_json::to_value(files::list(&dir, &path)?).map_err(|e| e.to_string())
            }))
            .await
        }
        Op::ReadTree { path } => {
            blocking(Box::new(move || {
                serde_json::to_value(files::read_tree(&dir, &path)?).map_err(|e| e.to_string())
            }))
            .await
        }
        Op::ReadFile { path } => {
            blocking(Box::new(move || {
                Ok(json!({ "content": files::read(&dir, &path)? }))
            }))
            .await
        }
        Op::WriteFile {
            path,
            content,
            validate,
        } => {
            if validate {
                let target = files::resolve(&dir, &path)?;
                let name = target
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if compose::is_compose_file(&name) {
                    let parent = target.parent().ok_or("invalid path")?.to_path_buf();
                    compose::validate(&ctx.docker_bin, &parent, &content).await?;
                }
            }
            let shown = format!(
                "{}/{}",
                dir.trim_end_matches('/'),
                path.trim_start_matches('/')
            );
            blocking(Box::new(move || {
                files::write(&dir, &path, &content).map(|_| json!({ "path": shown }))
            }))
            .await
        }
        Op::MakeDir { path } => {
            blocking(Box::new(move || {
                files::make_dir(&dir, &path).map(|_| Value::Null)
            }))
            .await
        }
        Op::Remove { path } => {
            blocking(Box::new(move || {
                files::remove(&dir, &path).map(|_| Value::Null)
            }))
            .await
        }
        Op::Rename { from, to } => {
            blocking(Box::new(move || {
                files::rename(&dir, &from, &to).map(|_| Value::Null)
            }))
            .await
        }
        Op::CreateStack { name, compose } => {
            let valid = !name.is_empty()
                && name.len() <= 64
                && name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric())
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if !valid {
                return Err("stack names use letters, digits, - and _".into());
            }
            let target = files::resolve(&dir, &name)?;
            if target.exists() {
                return Err("a folder with that name already exists".into());
            }
            compose::validate(&ctx.docker_bin, &target, &compose).await?;
            let shown = format!("{}/{}/docker-compose.yml", dir.trim_end_matches('/'), name);
            blocking(Box::new(move || {
                files::make_dir(&dir, &name)?;
                files::write(&dir, &format!("{name}/docker-compose.yml"), &compose)?;
                Ok(json!({ "path": shown }))
            }))
            .await
        }
        Op::Resources { kind } => {
            let to = |v: Result<Value, serde_json::Error>| v.map_err(|e| e.to_string());
            match kind {
                ResourceKind::Image => to(serde_json::to_value(resources::images(&ctx.docker).await?)),
                ResourceKind::Volume => to(serde_json::to_value(resources::volumes(&ctx.docker).await?)),
                ResourceKind::Network => to(serde_json::to_value(resources::networks(&ctx.docker).await?)),
            }
        }
        Op::RemoveResource { kind, id, force } => resources::remove(&ctx.docker, kind, &id, force).await.map(|_| Value::Null),
        _ => Err("unsupported operation".into()),
    }
}
