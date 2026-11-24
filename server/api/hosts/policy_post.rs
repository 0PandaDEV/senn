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
    state,
};

fn valid(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= 128
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[route(POST "/api/hosts/{agent}/policy")]
pub async fn policy(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let record = agent_record(cx).await?;
    let scope = body
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if scope.is_empty() || scope.split('/').count() > 2 || !scope.split('/').all(valid) {
        return Err(plain(400, "Invalid scope"));
    }
    let policy = match body.get("policy").and_then(Value::as_str).unwrap_or_default() {
        "" => None,
        p @ ("auto" | "notify" | "off") => Some(p),
        _ => return Err(plain(400, "Policy must be auto, notify, off or empty")),
    };
    if !state::set_update_policy(&record.id, scope, policy).await {
        return Err(plain(500, "Could not save the policy"));
    }
    if let Some(saved) = state::agent(&record.id).await {
        hub::push_config(&saved.id, saved.config);
    }
    Ok(json(&json!({ "ok": true })))
}
