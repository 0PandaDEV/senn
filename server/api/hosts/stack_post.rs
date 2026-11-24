use serde_json::json;
use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::{
    proto::{Op, StackAction},
    server::utils::{
        http_util::{query_map, start_job},
        logging::plain,
        params,
    },
};

#[route(POST "/api/hosts/{agent}/stacks/{stack}/{action}")]
pub async fn stack(cx: &Cx) -> Result<Response> {
    let action: StackAction = serde_json::from_value(json!(params::action(cx)))
        .map_err(|_| plain(400, "Unknown action"))?;
    let service = query_map(cx)
        .get("service")
        .cloned()
        .filter(|s| !s.is_empty());
    let op = Op::Stack {
        stack: params::stack(cx).to_string(),
        action,
        service,
    };
    start_job(cx, op).await
}
