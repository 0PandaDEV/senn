use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::server::utils::{
    http_util::{agent_record, json},
    hub,
    logging::plain,
    state::set_agent_settings,
};

#[route(POST "/api/agents/{agent}/config")]
pub async fn update(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let record = agent_record(cx).await?;
    let mut cfg = record.config.clone();
    let mut name = record.name.clone();
    if let Some(n) = body
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        name = n.chars().take(64).collect();
    }
    if let Some(policy) = body.get("defaultPolicy").and_then(Value::as_str) {
        if !matches!(policy, "notify" | "auto" | "off") {
            return Err(plain(400, "Default policy must be notify, auto or off"));
        }
        cfg.default_policy = policy.to_string();
        cfg.auto_update = policy == "auto";
    }
    if let Some(minutes) = body.get("updateIntervalMinutes").and_then(Value::as_u64) {
        cfg.update_interval_secs = minutes.clamp(5, 60 * 24 * 30) * 60;
    }
    let address = match body.get("address").and_then(Value::as_str).map(str::trim) {
        Some(a) => Some(a.trim_start_matches("http://").trim_start_matches("https://").trim_end_matches('/').to_string()).filter(|a| !a.is_empty()),
        None => record.address.clone(),
    };
    if address.as_deref().is_some_and(|a| a.contains('/') || a.contains(' ')) {
        return Err(plain(400, "The address should be a host name or IP, without a path"));
    }
    let saved = set_agent_settings(&record.id, &name, &cfg, address.as_deref())
        .await
        .ok_or_else(|| plain(404, "Unknown agent"))?;
    hub::push_config(&saved.id, saved.config);
    Ok(json(&json!({ "ok": true })))
}
