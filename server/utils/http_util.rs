use std::collections::HashMap;

use futures::{Stream, stream};
use http::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderValue, LOCATION},
};
use serde::Serialize;
use serde_json::{Value, json};
use topcoat::{
    context::Cx,
    router::{
        Body,
        content::sse::Event,
        request::{headers, uri},
        response::Response,
    },
};

use super::{
    hub::{self, StreamHandle, StreamMsg},
    jobs,
    logging::plain,
    params,
    state::{self, AgentRecord},
};
use crate::proto::Op;

pub fn json<T: Serialize>(value: &T) -> Response {
    let mut response = Response::new(Body::from(
        serde_json::to_string(value).unwrap_or_else(|_| "null".into()),
    ));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

pub fn redirect(to: &str) -> Response {
    let mut response = Response::new(Body::from(""));
    *response.status_mut() = StatusCode::FOUND;
    if let Ok(value) = HeaderValue::from_str(to) {
        response.headers_mut().insert(LOCATION, value);
    }
    response
}

pub fn query_map(cx: &Cx) -> HashMap<String, String> {
    let raw = uri(cx).query().unwrap_or("");
    reqwest::Url::parse(&format!("http://localhost/?{raw}"))
        .map(|u| {
            u.query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

pub async fn agent_record(cx: &Cx) -> topcoat::Result<AgentRecord> {
    state::agent(params::agent(cx))
        .await
        .ok_or_else(|| plain(404, "Unknown agent"))
}

fn hub_error(message: String) -> topcoat::Error {
    if message == "agent is offline" || message == "agent disconnected" {
        plain(409, "Agent is offline")
    } else {
        plain(400, &message)
    }
}

pub async fn agent_request(cx: &Cx, op: Op) -> topcoat::Result<Value> {
    let record = agent_record(cx).await?;
    hub::request(&record.id, op).await.map_err(hub_error)
}

pub async fn agent_stream(cx: &Cx, op: Op, cancel_on_drop: bool) -> topcoat::Result<StreamHandle> {
    let record = agent_record(cx).await?;
    hub::open_stream(&record.id, op, cancel_on_drop).map_err(hub_error)
}

pub fn sse_lines(handle: StreamHandle) -> impl Stream<Item = topcoat::Result<Event>> + use<> {
    stream::unfold(Some(handle), |state| async move {
        let mut handle = state?;
        match handle.rx.recv().await {
            Some(StreamMsg::Chunk(line)) => {
                Some((Ok(Event::new().event("line").data(line)), Some(handle)))
            }
            Some(StreamMsg::End(result)) => {
                let body = json!({ "ok": result.is_ok(), "error": result.err() });
                Some((Ok(Event::new().event("end").data(body.to_string())), None))
            }
            None => None,
        }
    })
}

pub fn headers_origin(cx: &Cx) -> String {
    let h = headers(cx);
    let host = h
        .get("x-forwarded-host")
        .or_else(|| h.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:3000");
    let proto = h
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    format!("{proto}://{host}")
}

pub async fn start_job(cx: &Cx, op: Op) -> topcoat::Result<Response> {
    let handle = agent_stream(cx, op, false).await?;
    let title = headers(cx)
        .get("x-job-title")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| reqwest::Url::parse(&format!("http://localhost/?t={v}")).ok())
        .and_then(|u| u.query_pairs().next().map(|(_, v)| v.into_owned()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Output".into());
    let key = headers(cx)
        .get("x-job-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let id = jobs::start(title, key, handle);
    Ok(json(&json!({ "id": id })))
}
