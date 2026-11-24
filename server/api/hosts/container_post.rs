use serde_json::json;
use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::{
    proto::{ContainerAction, Op},
    server::utils::{
        http_util::{agent_request, json},
        logging::plain,
        params,
    },
};

#[route(POST "/api/hosts/{agent}/containers/{container}/{action}")]
pub async fn container(cx: &Cx) -> Result<Response> {
    let action: ContainerAction = serde_json::from_value(json!(params::action(cx)))
        .map_err(|_| plain(400, "Unknown action"))?;
    let id = params::container(cx).to_string();
    agent_request(cx, Op::Container { id, action }).await?;
    Ok(json(&json!({ "ok": true })))
}
