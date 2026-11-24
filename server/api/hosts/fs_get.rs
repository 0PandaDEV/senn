use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::{
    proto::Op,
    server::utils::http_util::{agent_request, json, query_map},
};

#[route(GET "/api/hosts/{agent}/fs")]
pub async fn list(cx: &Cx) -> Result<Response> {
    let path = query_map(cx).get("path").cloned().unwrap_or_default();
    let entries = agent_request(cx, Op::ListDir { path }).await?;
    Ok(json(&entries))
}
