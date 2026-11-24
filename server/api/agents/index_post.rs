use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::{
    proto::AgentConfig,
    server::utils::{
        config::config,
        http_util::{headers_origin, json},
        logging::plain,
        state::create_agent,
    },
};

#[route(POST "/api/agents")]
pub async fn create(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.is_empty() || name.len() > 64 {
        return Err(plain(400, "Give the agent a name (max 64 characters)"));
    }
    let cfg = AgentConfig::default();
    let (record, token) = create_agent(&name, &cfg)
        .await
        .map_err(|e| plain(500, &e))?;
    let url = if config().base_url.is_empty() {
        headers_origin(cx)
    } else {
        config().base_url.clone()
    };
    Ok(json(
        &json!({ "id": record.id, "token": token, "url": url }),
    ))
}
