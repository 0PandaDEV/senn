use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::{proto::Op, server::utils::http_util::start_job};

#[route(POST "/api/hosts/{agent}/updates/apply")]
pub async fn apply(cx: &Cx) -> Result<Response> {
    start_job(cx, Op::UpdateAll).await
}

#[route(POST "/api/hosts/{agent}/updates/check")]
pub async fn check(cx: &Cx) -> Result<Response> {
    start_job(cx, Op::CheckUpdates).await
}
