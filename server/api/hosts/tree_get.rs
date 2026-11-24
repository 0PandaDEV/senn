use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::{
    proto::Op,
    server::utils::http_util::{agent_request, json, query_map},
};

#[route(GET "/api/hosts/{agent}/tree")]
pub async fn tree(cx: &Cx) -> Result<Response> {
    let path = query_map(cx).get("path").cloned().unwrap_or_default();
    let files = agent_request(cx, Op::ReadTree { path }).await?;
    Ok(json(&files))
}
