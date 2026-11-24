use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::server::utils::{http_util::json, logging::plain, state};

const MAX_AVATAR: usize = 512 * 1024;

#[route(POST "/api/account/profile")]
pub async fn profile(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let _ = cx;
    let name = body.get("display_name").and_then(Value::as_str).map(str::trim).unwrap_or_default();
    if name.chars().count() > 64 {
        return Err(plain(400, "Keep the name under 64 characters"));
    }
    if !state::set_display_name((!name.is_empty()).then_some(name)).await {
        return Err(plain(500, "Could not save"));
    }
    Ok(json(&json!({ "ok": true })))
}

#[route(POST "/api/account/avatar")]
pub async fn avatar(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let _ = cx;
    let data = body.get("data").and_then(Value::as_str).unwrap_or_default();
    if data.is_empty() {
        state::set_avatar(None).await;
        return Ok(json(&json!({ "ok": true })));
    }
    let (meta, payload) = data.split_once(',').ok_or_else(|| plain(400, "Not an image"))?;
    let kind = meta.strip_prefix("data:").and_then(|m| m.strip_suffix(";base64")).unwrap_or_default();
    if !matches!(kind, "image/png" | "image/jpeg" | "image/webp" | "image/gif") {
        return Err(plain(400, "Use a PNG, JPEG, WebP or GIF image"));
    }
    let bytes = STANDARD.decode(payload).map_err(|_| plain(400, "Not an image"))?;
    if bytes.len() > MAX_AVATAR {
        return Err(plain(400, "The image is too large, keep it under 512 KB"));
    }
    if !state::set_avatar(Some((&bytes, kind))).await {
        return Err(plain(500, "Could not save"));
    }
    Ok(json(&json!({ "ok": true })))
}
