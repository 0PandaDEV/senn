use serde_json::json;
use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::server::utils::{
    http_util::{agent_record, json},
    hub, state,
};

#[route(POST "/api/agents/{agent}/delete")]
pub async fn delete(cx: &Cx) -> Result<Response> {
    let record = agent_record(cx).await?;
    state::delete_agent(&record.id).await;
    hub::disconnect(&record.id);
    Ok(json(&json!({ "ok": true })))
}
