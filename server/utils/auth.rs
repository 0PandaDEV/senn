use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use topcoat::{
    context::Cx,
    cookie::{Cookie, Cookies, SameSite, cookies, time::Duration as CookieDuration},
    router::request::client_ip,
};

use super::{
    config::config,
    state::{self, add_session, hash, random_token, remove_session, session_valid},
};

pub const SESSION_COOKIE: &str = "senn_session";
const WEEK: i64 = 7 * 24 * 60 * 60;

#[derive(Clone, Copy)]
pub struct Authed;

static ATTEMPTS: LazyLock<Mutex<HashMap<String, Vec<Instant>>>> = LazyLock::new(Default::default);

pub fn throttled(cx: &Cx) -> bool {
    let key = client_ip(cx).map(|ip| ip.to_string()).unwrap_or_default();
    let mut map = ATTEMPTS.lock().expect("attempts lock");
    let list = map.entry(key).or_default();
    list.retain(|t| t.elapsed() < Duration::from_secs(60));
    list.push(Instant::now());
    list.len() > 8
}

pub async fn hash_password(password: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || bcrypt::hash(password, 10))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

pub async fn verify_login(username: String, password: String) -> bool {
    let Some(user) = state::user().await else {
        return false;
    };
    let hash = user.password_hash.clone();
    let password_ok =
        tokio::task::spawn_blocking(move || bcrypt::verify(password, &hash).unwrap_or(false))
            .await
            .unwrap_or(false);
    let a = Sha256::digest(username.as_bytes());
    let b = Sha256::digest(user.username.as_bytes());
    let name_ok: bool = a.ct_eq(&b).into();
    password_ok && name_ok
}

pub async fn start_session(cx: &Cx) {
    let token = random_token(32);
    add_session(&hash(&token), state::now() + WEEK).await;
    cookies(cx).add(
        Cookie::build((SESSION_COOKIE, token))
            .path("/")
            .max_age(CookieDuration::seconds(WEEK))
            .http_only(true)
            .secure(config().secure_cookies)
            .same_site(SameSite::Lax),
    );
}

pub fn read_cookie(cx: &Cx, name: &str) -> Option<String> {
    cookies(cx).get(name).map(|c| c.value().to_string())
}

pub async fn is_authed(cx: &Cx) -> bool {
    match read_cookie(cx, SESSION_COOKIE) {
        Some(t) => session_valid(&hash(&t)).await,
        None => false,
    }
}

pub async fn end_session(cx: &Cx) {
    if let Some(token) = read_cookie(cx, SESSION_COOKIE) {
        remove_session(&hash(&token)).await;
    }
    cookies(cx).remove(Cookie::build((SESSION_COOKIE, "")).path("/").build());
}
