use topcoat::{
    Result,
    context::Cx,
    router::{response::Response, route},
};

use crate::server::utils::{
    http_util::{json, query_map},
    logging::plain,
    metrics::{self, Query},
    state,
};

fn text(map: &std::collections::HashMap<String, String>, key: &str) -> Option<String> {
    map.get(key).cloned().filter(|v| !v.is_empty())
}

#[route(GET "/api/metrics/query")]
pub async fn query(cx: &Cx) -> Result<Response> {
    if !metrics::enabled() {
        return Err(plain(404, "Metrics storage is not configured"));
    }
    let q = query_map(cx);
    let host = text(&q, "host").ok_or_else(|| plain(400, "Missing host"))?;
    state::agent(&host)
        .await
        .ok_or_else(|| plain(404, "Unknown agent"))?;
    let now = state::now() as f64;
    let number = |key: &str| text(&q, key).and_then(|v| v.parse::<f64>().ok());
    let to = number("to").unwrap_or(now);
    let query = Query {
        host,
        metric: text(&q, "metric").ok_or_else(|| plain(400, "Missing metric"))?,
        from: number("from").unwrap_or(to - 3600.0),
        to,
        step: number("step").unwrap_or(1.0),
        container: text(&q, "container"),
        stack: text(&q, "stack"),
        service: text(&q, "service"),
        by: text(&q, "by"),
    };
    let result = metrics::query(query)
        .await
        .map_err(|(status, message)| plain(status, &message))?;
    Ok(json(&result))
}
