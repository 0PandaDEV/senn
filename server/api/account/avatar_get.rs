use http::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue};
use topcoat::{
    Result,
    context::Cx,
    router::{Body, response::Response, route},
};

use crate::server::utils::{logging::plain, state};

#[route(GET "/api/account/avatar")]
pub async fn avatar(cx: &Cx) -> Result<Response> {
    let _ = cx;
    let (bytes, kind) = state::avatar().await.ok_or_else(|| plain(404, "No avatar"))?;
    let mut response = Response::new(Body::from(bytes));
    if let Ok(v) = HeaderValue::from_str(&kind) {
        response.headers_mut().insert(CONTENT_TYPE, v);
    }
    response.headers_mut().insert(CACHE_CONTROL, HeaderValue::from_static("private, max-age=31536000"));
    Ok(response)
}
