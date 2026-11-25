use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex, RwLock},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;

use super::{config::config, hub, jobs, state};

mod services;
mod target;

const KEY: &str = "notify";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifyConfig {
    pub enabled: bool,
    pub targets: String,
    #[serde(skip_serializing)]
    pub url: String,
    #[serde(skip_serializing)]
    pub topic: String,
    #[serde(skip_serializing)]
    pub token: String,
    pub agent_offline: bool,
    pub agent_online: bool,
    pub container_crash: bool,
    pub container_unhealthy: bool,
    pub job_failed: bool,
    pub job_done: bool,
    pub updates: bool,
    pub auto_update: bool,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            targets: String::new(),
            url: String::new(),
            topic: String::new(),
            token: String::new(),
            agent_offline: true,
            agent_online: true,
            container_crash: true,
            container_unhealthy: true,
            job_failed: true,
            job_done: false,
            updates: true,
            auto_update: true,
        }
    }
}

static CONFIG: LazyLock<RwLock<NotifyConfig>> = LazyLock::new(Default::default);
static DOWN: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);
static UPDATES: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(Default::default);

pub fn current() -> NotifyConfig {
    CONFIG.read().expect("notify lock").clone()
}

pub async fn save(cfg: NotifyConfig) -> bool {
    let Ok(text) = serde_json::to_string(&cfg) else {
        return false;
    };
    let ok = state::set_setting(KEY, &text).await;
    if ok {
        *CONFIG.write().expect("notify lock") = cfg;
    }
    ok
}

#[derive(Debug, Clone)]
pub struct Message {
    pub title: String,
    pub body: String,
    pub priority: u8,
    pub tags: &'static str,
    pub click: Option<String>,
}

impl Message {
    pub fn text(&self) -> String {
        match &self.click {
            Some(c) => format!("{}\n{c}", self.body),
            None => self.body.clone(),
        }
    }

    pub fn full(&self) -> String {
        format!("{}\n{}", self.title, self.text())
    }

    pub fn icon(&self) -> Option<String> {
        link("/icon.png")
    }

    pub fn kind(&self) -> &'static str {
        match self.tags {
            "white_check_mark" | "green_circle" => "success",
            "warning" => "warning",
            _ if self.priority >= 4 => "failure",
            _ => "info",
        }
    }
}

impl NotifyConfig {
    pub fn urls(&self) -> Vec<String> {
        self.targets
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect()
    }

    fn migrate(&mut self) {
        if self.targets.trim().is_empty() && !self.topic.trim().is_empty() {
            let host = self.url.trim().trim_end_matches('/');
            let (scheme, host) = match host.split_once("://") {
                Some(("http", h)) => ("ntfy", h),
                Some((_, h)) => ("ntfys", h),
                None => ("ntfys", "ntfy.sh"),
            };
            let host = if host.is_empty() { "ntfy.sh" } else { host };
            let token = if self.token.trim().is_empty() {
                String::new()
            } else {
                format!("?token={}", self.token.trim())
            };
            self.targets = format!("{scheme}://{host}/{}{token}", self.topic.trim());
        }
    }
}

pub use services::SERVICES;

pub fn validate(raw: &str) -> Result<(), String> {
    let t = target::parse(raw)?;
    if services::find(&t.scheme).is_none() {
        return Err(format!("unsupported notification service: {}://", t.scheme));
    }
    if t.host.is_empty() {
        return Err(format!("missing details in {raw}"));
    }
    Ok(())
}

async fn deliver(raw: &str, msg: &Message) -> Result<(), String> {
    services::deliver(target::parse(raw)?, msg.clone()).await
}

pub async fn send(cfg: &NotifyConfig, msg: Message) -> Result<(), String> {
    let urls = cfg.urls();
    if urls.is_empty() {
        return Err("Add at least one notification URL".into());
    }
    let mut errors = Vec::new();
    for u in &urls {
        if let Err(e) = deliver(u, &msg).await {
            errors.push(e);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn link(path: &str) -> Option<String> {
    let base = config().base_url.trim_end_matches('/');
    (!base.is_empty()).then(|| format!("{base}{path}"))
}

fn dispatch(enabled: impl Fn(&NotifyConfig) -> bool, msg: Message) {
    let cfg = current();
    if !cfg.enabled || !enabled(&cfg) {
        return;
    }
    tokio::spawn(async move {
        if let Err(e) = send(&cfg, msg).await {
            tracing::warn!("notification failed: {e}");
        }
    });
}

async fn agent_name(id: &str) -> String {
    state::agent(id)
        .await
        .map(|a| a.name)
        .unwrap_or_else(|| id.to_string())
}

async fn on_hub(agent: String, kind: String, data: Value) {
    match kind.as_str() {
        "offline" => {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if hub::online(&agent) {
                return;
            }
            DOWN.lock().expect("down lock").insert(agent.clone());
            let name = agent_name(&agent).await;
            dispatch(
                |c| c.agent_offline,
                Message {
                    title: format!("{name} is offline"),
                    body: "The agent lost its connection to the panel.".into(),
                    priority: 4,
                    tags: "red_circle",
                    click: link(&format!("/hosts/{agent}")),
                },
            );
        }
        "online" => {
            if !DOWN.lock().expect("down lock").remove(&agent) {
                return;
            }
            let name = agent_name(&agent).await;
            dispatch(
                |c| c.agent_online,
                Message {
                    title: format!("{name} is back online"),
                    body: "The agent reconnected.".into(),
                    priority: 3,
                    tags: "green_circle",
                    click: link(&format!("/hosts/{agent}")),
                },
            );
        }
        "docker" if data.get("notable").and_then(Value::as_bool) == Some(true) => {
            let s = |k: &str| {
                data.get(k)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let action = s("action");
            let host = agent_name(&agent).await;
            let what = match (
                data.get("stack").and_then(Value::as_str),
                data.get("service").and_then(Value::as_str),
            ) {
                (Some(st), Some(sv)) => format!("{st}/{sv}"),
                _ => s("name"),
            };
            let click = link(&format!("/hosts/{agent}/containers/{}", s("id")));
            if action == "oom" {
                dispatch(
                    |c| c.container_crash,
                    Message {
                        title: format!("{what} ran out of memory"),
                        body: format!("On {host}. Image {}.", s("image")),
                        priority: 4,
                        tags: "boom",
                        click,
                    },
                );
            } else if action == "die" {
                dispatch(
                    |c| c.container_crash,
                    Message {
                        title: format!("{what} crashed"),
                        body: format!("Exited with code {} on {host}.", s("exit_code")),
                        priority: 4,
                        tags: "x",
                        click,
                    },
                );
            } else {
                dispatch(
                    |c| c.container_unhealthy,
                    Message {
                        title: format!("{what} is unhealthy"),
                        body: format!("Health check failing on {host}."),
                        priority: 4,
                        tags: "warning",
                        click,
                    },
                );
            }
        }
        "updates" => {
            let Ok(st) = hub::host_state(&agent).await else {
                return;
            };
            let available: Vec<String> = st.pending_images();
            let before = UPDATES
                .lock()
                .expect("updates lock")
                .insert(agent.clone(), available.len());
            if available.is_empty() || before.is_none_or(|b| b >= available.len()) {
                return;
            }
            let name = agent_name(&agent).await;
            let mut body = available
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            if available.len() > 8 {
                body.push_str(&format!("\n+{} more", available.len() - 8));
            }
            dispatch(
                |c| c.updates,
                Message {
                    title: format!(
                        "{} image update{} on {name}",
                        available.len(),
                        if available.len() == 1 { "" } else { "s" }
                    ),
                    body,
                    priority: 3,
                    tags: "arrow_up",
                    click: link(&format!("/hosts/{agent}/stacks")),
                },
            );
        }
        "auto_update" => {
            let s = |k: &str| {
                data.get(k)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let ok = data.get("ok").and_then(Value::as_bool).unwrap_or(false);
            let name = agent_name(&agent).await;
            let what = format!("{}/{}", s("stack"), s("service"));
            dispatch(
                |c| c.auto_update,
                Message {
                    title: if ok {
                        format!("Updated {what}")
                    } else {
                        format!("Auto update of {what} failed")
                    },
                    body: if ok {
                        format!("Pulled and recreated on {name}.")
                    } else {
                        format!("{} on {name}.", s("error"))
                    },
                    priority: if ok { 2 } else { 4 },
                    tags: if ok { "white_check_mark" } else { "x" },
                    click: link(&format!("/hosts/{agent}/stacks/{}", s("stack"))),
                },
            );
        }
        _ => {}
    }
}

fn on_job(v: Value) {
    if v.get("ev").and_then(Value::as_str) != Some("end") {
        return;
    }
    let title = v
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Job")
        .to_string();
    if v.get("ok").and_then(Value::as_bool) == Some(true) {
        dispatch(
            |c| c.job_done,
            Message {
                title: format!("{title} finished"),
                body: "Completed without errors.".into(),
                priority: 2,
                tags: "white_check_mark",
                click: None,
            },
        );
    } else {
        let error = v
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("failed")
            .to_string();
        dispatch(
            |c| c.job_failed,
            Message {
                title: format!("{title} failed"),
                body: error,
                priority: 4,
                tags: "x",
                click: None,
            },
        );
    }
}

pub async fn start() {
    if let Some(mut cfg) = state::setting(KEY)
        .await
        .and_then(|t| serde_json::from_str::<NotifyConfig>(&t).ok())
    {
        cfg.migrate();
        *CONFIG.write().expect("notify lock") = cfg;
    }
    tokio::spawn(async {
        let mut events = hub::subscribe();
        let mut job_events = jobs::subscribe();
        loop {
            tokio::select! {
                e = events.recv() => match e {
                    Ok(e) if e.kind != "stats" => {
                        tokio::spawn(on_hub(e.agent, e.kind, e.data));
                    }
                    Ok(_) | Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                },
                e = job_events.recv() => match e {
                    Ok(v) => on_job(v),
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                },
            }
        }
    });
}
