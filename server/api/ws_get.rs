use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast::error::RecvError;
use topcoat::{
    Result,
    context::Cx,
    router::{
        content::websocket::{Message, WebSocketUpgrade},
        response::Response,
        route,
    },
};

use crate::server::utils::{hub, jobs, metrics};

#[route(GET "/api/ws")]
pub async fn ws(cx: &Cx, upgrade: WebSocketUpgrade) -> Result<Response> {
    let _ = cx;
    upgrade.on_upgrade(|socket| async move {
        let (mut sink, mut stream) = socket.split();
        let mut events = hub::subscribe();
        let mut job_events = jobs::subscribe();
        let mut samples = metrics::subscribe();
        let mut sub: Option<String> = None;
        let mut ping = tokio::time::interval(std::time::Duration::from_secs(25));
        loop {
            let text = tokio::select! {
                e = events.recv() => match e {
                    Ok(e) => serde_json::json!({ "t": "hub", "agent": e.agent, "kind": e.kind, "data": e.data }).to_string(),
                    Err(RecvError::Lagged(_)) => serde_json::json!({ "t": "hub", "kind": "lagged" }).to_string(),
                    Err(RecvError::Closed) => break,
                },
                e = job_events.recv() => match e {
                    Ok(v) => v.to_string(),
                    Err(RecvError::Lagged(_)) => serde_json::json!({ "t": "jobs" }).to_string(),
                    Err(RecvError::Closed) => break,
                },
                s = samples.recv() => match s {
                    Ok(t) if sub.as_deref() == Some(t.agent.as_str()) => t.to_json().to_string(),
                    Ok(_) | Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                },
                _ = ping.tick() => serde_json::json!({ "t": "ping" }).to_string(),
                frame = stream.next() => match frame {
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t)
                            && v["t"] == "sub"
                        {
                            sub = v["agent"].as_str().map(String::from);
                        }
                        continue;
                    }
                    _ => continue,
                },
            };
            if sink.send(Message::text(text)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    })
}
