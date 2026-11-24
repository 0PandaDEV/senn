use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::server::utils::{
    auth::{start_session, throttled, verify_login},
    http_util::json,
    logging::plain,
};

#[route(POST "/api/auth/login")]
pub async fn login(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
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
    if password.is_empty() || !verify_login(username, password).await {
        return Err(plain(401, "Wrong username or password"));
    }
    start_session(cx).await;
    Ok(json(&json!({ "ok": true })))
}
