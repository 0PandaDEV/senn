use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::{
    proto::{Op, ResourceKind},
    server::utils::{
        http_util::{agent_request, json, start_job},
        logging::plain,
        params,
    },
};

fn kind(cx: &Cx) -> Result<ResourceKind> {
    match params::kind(cx) {
        "images" => Ok(ResourceKind::Image),
        "volumes" => Ok(ResourceKind::Volume),
        "networks" => Ok(ResourceKind::Network),
        _ => Err(plain(404, "Unknown resource")),
    }
}

#[route(POST "/api/hosts/{agent}/resources/{kind}/{action}")]
pub async fn resource(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let kind = kind(cx)?;
    let op = match params::action(cx) {
        "remove" => {
            let id = body.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
            if id.is_empty() {
                return Err(plain(400, "Missing id"));
            }
            Op::RemoveResource { kind, id, force: body.get("force").and_then(Value::as_bool).unwrap_or(false) }
        }
        "prune" => return start_job(cx, Op::Prune { kind }).await,
        _ => return Err(plain(404, "Unknown action")),
    };
    let data = agent_request(cx, op).await?;
    Ok(json(&json!({ "ok": true, "data": data })))
}
