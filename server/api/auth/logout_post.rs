use serde_json::json;
use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::server::utils::{auth::end_session, http_util::json};

#[route(POST "/api/auth/logout")]
pub async fn logout(cx: &Cx) -> Result<Response> {
    end_session(cx).await;
    Ok(json(&json!({ "ok": true })))
}
