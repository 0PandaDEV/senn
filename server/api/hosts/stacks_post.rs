use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::{
    proto::Op,
    server::utils::{
        http_util::{agent_request, json},
        logging::plain,
    },
};

#[route(POST "/api/hosts/{agent}/stacks")]
pub async fn create(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let compose = body
        .get("compose")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if name.is_empty() || compose.trim().is_empty() {
        return Err(plain(400, "Name and compose content are required"));
    }
    let saved = agent_request(cx, Op::CreateStack { name, compose }).await?;
    Ok(json(&json!({ "ok": true, "path": saved.get("path") })))
}
