use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::server::utils::{
    auth::{hash_password, start_session, throttled},
    http_util::json,
    logging::plain,
    state::{create_user, has_user},
};

#[route(POST "/api/auth/setup")]
pub async fn setup(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    if has_user().await {
        return Err(plain(409, "Senn is already set up"));
    }
    if throttled(cx) {
        return Err(plain(429, "Too many attempts, wait a minute"));
    }
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let confirm = body
        .get("confirm")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if username.len() < 2 || username.len() > 64 {
        return Err(plain(400, "Username needs 2 to 64 characters"));
    }
    if password.len() < 10 {
        return Err(plain(400, "Password needs at least 10 characters"));
    }
    if password != confirm {
        return Err(plain(400, "Passwords do not match"));
    }
    let hash = hash_password(password).await.map_err(|e| plain(500, &e))?;
    if !create_user(&username, &hash).await {
        return Err(plain(409, "Senn is already set up"));
    }
    start_session(cx).await;
    Ok(json(&json!({ "ok": true })))
}
