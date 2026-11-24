use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex, RwLock},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;

use super::{config::config, hub, jobs, state};

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
    let Ok(text) = serde_json::to_string(&cfg) else { return false };
    let ok = state::set_setting(KEY, &text).await;
    if ok {
        *CONFIG.write().expect("notify lock") = cfg;
    }
    ok
}

pub struct Message {
    pub title: String,
    pub body: String,
    pub priority: u8,
    pub tags: &'static str,
    pub click: Option<String>,
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
            let token = if self.token.trim().is_empty() { String::new() } else { format!("?token={}", self.token.trim()) };
            self.targets = format!("{scheme}://{host}/{}{token}", self.topic.trim());
        }
    }
}

pub const SCHEMES: &[(&str, &str)] = &[
    ("ntfy", "ntfys://topic · ntfys://host/topic?token=tk_… · ntfy://host/topic for http"),
    ("Discord", "discord://webhook_id/webhook_token"),
    ("Telegram", "tgram://bot_token/chat_id"),
    ("Slack", "slack://TokenA/TokenB/TokenC"),
    ("Gotify", "gotifys://host/app_token"),
    ("Pushover", "pover://user_key@app_token"),
    ("Mattermost", "mmosts://host/hook_id"),
    ("Pushbullet", "pbul://access_token"),
    ("Webhook (JSON)", "jsons://host/path · json:// for http"),
];

struct Target {
    scheme: String,
    user: Option<String>,
    pass: Option<String>,
    host: String,
    path: Vec<String>,
    query: HashMap<String, String>,
}

fn parse(raw: &str) -> Result<Target, String> {
    let (scheme, rest) = raw.split_once("://").ok_or_else(|| format!("not a notification URL: {raw}"))?;
    let (rest, query) = match rest.split_once('?') {
        Some((r, q)) => (r, q),
        None => (rest, ""),
    };
    let query = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_lowercase(), v.to_string()))
        .collect();
    let (auth, rest) = match rest.rsplit_once('@') {
        Some((a, r)) => (Some(a), r),
        None => (None, rest),
    };
    let (user, pass) = match auth.map(|a| a.split_once(':')) {
        Some(Some((u, p))) => (Some(u.to_string()), Some(p.to_string())),
        Some(None) => (auth.map(String::from), None),
        None => (None, None),
    };
    let mut parts = rest.split('/').filter(|p| !p.is_empty()).map(String::from);
    let host = parts.next().unwrap_or_default();
    Ok(Target { scheme: scheme.to_lowercase(), user, pass, host, path: parts.collect(), query })
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().timeout(Duration::from_secs(15)).build().unwrap_or_default()
}

fn text(msg: &Message) -> String {
    match &msg.click {
        Some(c) => format!("{}\n{c}", msg.body),
        None => msg.body.clone(),
    }
}

async fn check(res: Result<reqwest::Response, reqwest::Error>, name: &str) -> Result<(), String> {
    let res = res.map_err(|e| format!("{name}: {e}"))?;
    if res.status().is_success() {
        return Ok(());
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    Err(format!("{name} answered {status}: {}", body.chars().take(160).collect::<String>()))
}

async fn deliver(raw: &str, msg: &Message) -> Result<(), String> {
    let t = parse(raw)?;
    let http = |secure: bool| if secure { "https" } else { "http" };
    let first = || t.path.first().cloned().unwrap_or_default();
    match t.scheme.as_str() {
        "ntfy" | "ntfys" => {
            let (base, topic) = if t.path.is_empty() {
                ("https://ntfy.sh".to_string(), t.host.clone())
            } else {
                (format!("{}://{}", http(t.scheme == "ntfys"), t.host), t.path.join("/"))
            };
            let mut req = client()
                .post(format!("{base}/{topic}"))
                .header("Title", msg.title.clone())
                .header("Priority", msg.priority.to_string())
                .header("Tags", msg.tags)
                .body(msg.body.clone());
            if let Some(c) = &msg.click {
                req = req.header("Click", c.clone());
            }
            if let Some(token) = t.query.get("token") {
                req = req.bearer_auth(token);
            } else if let (Some(u), Some(p)) = (&t.user, &t.pass) {
                req = req.basic_auth(u, Some(p));
            }
            check(req.send().await, "ntfy").await
        }
        "discord" => {
            let url = format!("https://discord.com/api/webhooks/{}/{}", t.host, first());
            let body = serde_json::json!({ "embeds": [{ "title": msg.title, "description": text(msg), "color": if msg.priority >= 4 { 0xe5484d } else { 0x46c27f } }] });
            check(client().post(url).json(&body).send().await, "discord").await
        }
        "tgram" => {
            let mut last = Ok(());
            for chat in &t.path {
                let body = serde_json::json!({ "chat_id": chat, "text": format!("{}\n{}", msg.title, text(msg)), "disable_web_page_preview": true });
                last = check(client().post(format!("https://api.telegram.org/bot{}/sendMessage", t.host)).json(&body).send().await, "telegram").await;
                last.clone()?;
            }
            last
        }
        "slack" => {
            let url = format!("https://hooks.slack.com/services/{}/{}", t.host, t.path.join("/"));
            let body = serde_json::json!({ "text": format!("*{}*\n{}", msg.title, text(msg)) });
            check(client().post(url).json(&body).send().await, "slack").await
        }
        "gotify" | "gotifys" => {
            let token = t.path.last().cloned().unwrap_or_default();
            let prefix = t.path[..t.path.len().saturating_sub(1)].join("/");
            let base = if prefix.is_empty() { t.host.clone() } else { format!("{}/{prefix}", t.host) };
            let url = format!("{}://{base}/message?token={token}", http(t.scheme == "gotifys"));
            let body = serde_json::json!({ "title": msg.title, "message": text(msg), "priority": msg.priority as u32 * 2 });
            check(client().post(url).json(&body).send().await, "gotify").await
        }
        "pover" => {
            let user = t.user.clone().unwrap_or_default();
            let body = serde_json::json!({
                "token": t.host,
                "user": user,
                "title": msg.title,
                "message": msg.body,
                "priority": (msg.priority as i32 - 3).clamp(-1, 1),
                "url": msg.click,
            });
            check(client().post("https://api.pushover.net/1/messages.json").json(&body).send().await, "pushover").await
        }
        "mmost" | "mmosts" => {
            let url = format!("{}://{}/hooks/{}", http(t.scheme == "mmosts"), t.host, t.path.join("/"));
            let body = serde_json::json!({ "text": format!("**{}**\n{}", msg.title, text(msg)) });
            check(client().post(url).json(&body).send().await, "mattermost").await
        }
        "pbul" => {
            let body = serde_json::json!({ "type": "note", "title": msg.title, "body": text(msg) });
            check(client().post("https://api.pushbullet.com/v2/pushes").header("Access-Token", t.host.clone()).json(&body).send().await, "pushbullet").await
        }
        "json" | "jsons" => {
            let url = format!("{}://{}/{}", http(t.scheme == "jsons"), t.host, t.path.join("/"));
            let kind = if msg.priority >= 4 { "failure" } else { "info" };
            let body = serde_json::json!({ "title": msg.title, "message": msg.body, "type": kind, "url": msg.click });
            let mut req = client().post(url).json(&body);
            if let (Some(u), p) = (&t.user, &t.pass) {
                req = req.basic_auth(u, p.as_ref());
            }
            check(req.send().await, "webhook").await
        }
        other => Err(format!("unsupported notification service: {other}://")),
    }
}

pub fn validate(raw: &str) -> Result<(), String> {
    let t = parse(raw)?;
    if !SCHEMES_KEYS.contains(&t.scheme.as_str()) {
        return Err(format!("unsupported notification service: {}://", t.scheme));
    }
    if t.host.is_empty() {
        return Err(format!("missing details in {raw}"));
    }
    Ok(())
}

const SCHEMES_KEYS: &[&str] = &["ntfy", "ntfys", "discord", "tgram", "slack", "gotify", "gotifys", "pover", "mmost", "mmosts", "pbul", "json", "jsons"];

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
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
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
    state::agent(id).await.map(|a| a.name).unwrap_or_else(|| id.to_string())
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
            let s = |k: &str| data.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            let action = s("action");
            let host = agent_name(&agent).await;
            let what = match (data.get("stack").and_then(Value::as_str), data.get("service").and_then(Value::as_str)) {
                (Some(st), Some(sv)) => format!("{st}/{sv}"),
                _ => s("name"),
            };
            let click = link(&format!("/hosts/{agent}/containers/{}", s("id")));
            if action == "oom" {
                dispatch(
                    |c| c.container_crash,
                    Message { title: format!("{what} ran out of memory"), body: format!("On {host}. Image {}.", s("image")), priority: 4, tags: "boom", click },
                );
            } else if action == "die" {
                dispatch(
                    |c| c.container_crash,
                    Message { title: format!("{what} crashed"), body: format!("Exited with code {} on {host}.", s("exit_code")), priority: 4, tags: "x", click },
                );
            } else {
                dispatch(
                    |c| c.container_unhealthy,
                    Message { title: format!("{what} is unhealthy"), body: format!("Health check failing on {host}."), priority: 4, tags: "warning", click },
                );
            }
        }
        "updates" => {
            let Ok(st) = hub::host_state(&agent).await else { return };
            let available: Vec<String> = st.pending_images();
            let before = UPDATES.lock().expect("updates lock").insert(agent.clone(), available.len());
            if available.is_empty() || before.is_none_or(|b| b >= available.len()) {
                return;
            }
            let name = agent_name(&agent).await;
            let mut body = available.iter().take(8).cloned().collect::<Vec<_>>().join("\n");
            if available.len() > 8 {
                body.push_str(&format!("\n+{} more", available.len() - 8));
            }
            dispatch(
                |c| c.updates,
                Message {
                    title: format!("{} image update{} on {name}", available.len(), if available.len() == 1 { "" } else { "s" }),
                    body,
                    priority: 3,
                    tags: "arrow_up",
                    click: link(&format!("/hosts/{agent}/stacks")),
                },
            );
        }
        "auto_update" => {
            let s = |k: &str| data.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            let ok = data.get("ok").and_then(Value::as_bool).unwrap_or(false);
            let name = agent_name(&agent).await;
            let what = format!("{}/{}", s("stack"), s("service"));
            dispatch(
                |c| c.auto_update,
                Message {
                    title: if ok { format!("Updated {what}") } else { format!("Auto update of {what} failed") },
                    body: if ok { format!("Pulled and recreated on {name}.") } else { format!("{} on {name}.", s("error")) },
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
    let title = v.get("title").and_then(Value::as_str).unwrap_or("Job").to_string();
    if v.get("ok").and_then(Value::as_bool) == Some(true) {
        dispatch(|c| c.job_done, Message { title: format!("{title} finished"), body: "Completed without errors.".into(), priority: 2, tags: "white_check_mark", click: None });
    } else {
        let error = v.get("error").and_then(Value::as_str).unwrap_or("failed").to_string();
        dispatch(|c| c.job_failed, Message { title: format!("{title} failed"), body: error, priority: 4, tags: "x", click: None });
    }
}

pub async fn start() {
    if let Some(mut cfg) = state::setting(KEY).await.and_then(|t| serde_json::from_str::<NotifyConfig>(&t).ok()) {
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
