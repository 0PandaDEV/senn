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

fn field(body: &Value, key: &str) -> Result<String> {
    body.get(key)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| plain(400, &format!("{key} is required")))
}

#[route(POST "/api/hosts/{agent}/fs/mkdir")]
pub async fn mkdir(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    agent_request(
        cx,
        Op::MakeDir {
            path: field(&body, "path")?,
        },
    )
    .await?;
    Ok(json(&json!({ "ok": true })))
}

#[route(POST "/api/hosts/{agent}/fs/remove")]
pub async fn remove(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    agent_request(
        cx,
        Op::Remove {
            path: field(&body, "path")?,
        },
    )
    .await?;
    Ok(json(&json!({ "ok": true })))
}

#[route(POST "/api/hosts/{agent}/fs/rename")]
pub async fn rename(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    agent_request(
        cx,
        Op::Rename {
            from: field(&body, "from")?,
            to: field(&body, "to")?,
        },
    )
    .await?;
    Ok(json(&json!({ "ok": true })))
}
