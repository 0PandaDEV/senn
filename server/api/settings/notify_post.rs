use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::server::utils::{
    http_util::json,
    logging::plain,
    notify::{self, Message, NotifyConfig},
};

fn flag(body: &Value, key: &str) -> bool {
    matches!(body.get(key), Some(Value::Bool(true)))
        || body
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|v| v == "on" || v == "true")
}

#[route(POST "/api/settings/notify")]
pub async fn save(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let _ = cx;
    let targets = body
        .get("targets")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let enabled = flag(&body, "enabled");
    let probe = NotifyConfig {
        targets: targets.clone(),
        ..Default::default()
    };
    for u in probe.urls() {
        notify::validate(&u).map_err(|e| plain(400, &e))?;
    }
    if enabled && probe.urls().is_empty() {
        return Err(plain(
            400,
            "Add at least one notification URL to turn notifications on",
        ));
    }
    let cfg = NotifyConfig {
        enabled,
        targets,
        agent_offline: flag(&body, "agent_offline"),
        agent_online: flag(&body, "agent_online"),
        container_crash: flag(&body, "container_crash"),
        container_unhealthy: flag(&body, "container_unhealthy"),
        job_failed: flag(&body, "job_failed"),
        job_done: flag(&body, "job_done"),
        updates: flag(&body, "updates"),
        auto_update: flag(&body, "auto_update"),
        ..Default::default()
    };
    if !notify::save(cfg).await {
        return Err(plain(500, "Could not save the settings"));
    }
    Ok(json(&json!({ "ok": true })))
}

#[route(POST "/api/settings/notify/test")]
pub async fn test(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let _ = cx;
    let mut cfg = notify::current();
    if let Some(targets) = body.get("targets").and_then(Value::as_str) {
        cfg.targets = targets.trim().to_string();
        for u in cfg.urls() {
            notify::validate(&u).map_err(|e| plain(400, &e))?;
        }
    }
    notify::send(
        &cfg,
        Message {
            title: "Senn test notification".into(),
            body: "Notifications from this panel reach you.".into(),
            priority: 3,
            tags: "bell",
            click: None,
        },
    )
    .await
    .map_err(|e| plain(400, &e))?;
    Ok(json(&json!({ "ok": true })))
}
