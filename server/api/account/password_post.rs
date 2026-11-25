use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::server::utils::{
    auth::{hash_password, start_session, throttled, verify_password},
    http_util::json,
    logging::plain,
    state,
};

#[route(POST "/api/account/password")]
pub async fn password(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    if throttled(cx) {
        return Err(plain(429, "Too many attempts, wait a minute"));
    }
    let field = |name: &str| {
        body.get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let (current, password, confirm) = (field("current"), field("password"), field("confirm"));
    if !verify_password(current).await {
        return Err(plain(400, "The current password is wrong"));
    }
    if password.len() < 10 {
        return Err(plain(400, "The new password needs at least 10 characters"));
    }
    if password != confirm {
        return Err(plain(400, "The new passwords do not match"));
    }
    let hash = hash_password(password).await.map_err(|e| plain(500, &e))?;
    if !state::set_password(&hash).await {
        return Err(plain(500, "Could not save"));
    }
    state::clear_sessions().await;
    start_session(cx).await;
    Ok(json(&json!({ "ok": true })))
}
