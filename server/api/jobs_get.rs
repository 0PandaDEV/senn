use topcoat::{
    Result,
    context::Cx,
    router::{path_param, response::Response, route},
};

use crate::server::utils::{http_util::json, jobs, logging::plain};

path_param!(pub job);

#[route(GET "/api/jobs")]
pub async fn list(cx: &Cx) -> Result<Response> {
    let _ = cx;
    Ok(json(&jobs::snapshot()))
}

#[route(POST "/api/jobs/{job}/dismiss")]
pub async fn dismiss(cx: &Cx) -> Result<Response> {
    let id: u64 = job(cx).parse().map_err(|_| plain(400, "Invalid job"))?;
    jobs::dismiss(id);
    Ok(json(&serde_json::json!({ "ok": true })))
}
