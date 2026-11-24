use topcoat::{
    Result,
    context::Cx,
    router::{content::websocket::WebSocketUpgrade, request::headers, response::Response, route},
};

use crate::server::utils::{hub, logging::plain, state::verify_agent_token};

#[route(GET "/api/agent/connect")]
pub async fn connect(cx: &Cx, upgrade: WebSocketUpgrade) -> Result<Response> {
    let token = headers(cx)
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string)
        .ok_or_else(|| plain(401, "Unauthorized"))?;
    let id = verify_agent_token(&token)
        .await
        .ok_or_else(|| plain(401, "Unauthorized"))?;
    upgrade.on_upgrade(move |socket| async move { hub::serve(id, socket).await })
}
