use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::{
    proto::Op,
    server::utils::{
        http_util::{agent_request, json, query_map},
        logging::plain,
    },
};

#[route(GET "/api/hosts/{agent}/file")]
pub async fn read(cx: &Cx) -> Result<Response> {
    let path = query_map(cx)
        .get("path")
        .cloned()
        .ok_or_else(|| plain(400, "path is required"))?;
    let data = agent_request(cx, Op::ReadFile { path }).await?;
    Ok(json(&data))
}
