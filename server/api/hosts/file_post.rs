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

#[route(POST "/api/hosts/{agent}/file")]
pub async fn write(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let path = body
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| plain(400, "path is required"))?
        .to_string();
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| plain(400, "content is required"))?
        .to_string();
    let validate = body
        .get("validate")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let saved = agent_request(
        cx,
        Op::WriteFile {
            path,
            content,
            validate,
        },
    )
    .await?;
    Ok(json(&json!({ "ok": true, "path": saved.get("path") })))
}
