use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::server::utils::{http_util::json, jobs};

#[route(GET "/api/jobs")]
pub async fn list(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(json(&jobs::snapshot()))
}
