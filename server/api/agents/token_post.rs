use serde_json::json;
use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::server::utils::{
    config::config,
    http_util::{agent_record, headers_origin, json},
    hub,
    logging::plain,
    state::rotate_agent_token,
};

#[route(POST "/api/agents/{agent}/token")]
pub async fn rotate(cx: &Cx) -> Result<Response> {
    let record = agent_record(cx).await?;
    let token = rotate_agent_token(&record.id)
        .await
        .ok_or_else(|| plain(404, "Unknown agent"))?;
    hub::disconnect(&record.id);
    let url = if config().base_url.is_empty() {
        headers_origin(cx)
    } else {
        config().base_url.clone()
    };
    Ok(json(
        &json!({ "id": record.id, "token": token, "url": url }),
    ))
}
