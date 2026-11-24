use futures::Stream;
use topcoat::{
    Result,
    context::Cx,
    router::{
        content::sse::{Event, KeepAlive, Sse},
        route,
    },
};

use crate::{
    proto::Op,
    server::utils::http_util::{agent_stream, query_map, sse_lines},
};

#[route(GET "/api/hosts/{agent}/logs")]
pub async fn logs(cx: &Cx) -> Result<Sse<impl Stream<Item = Result<Event>> + use<>>> {
    let q = query_map(cx);
    let op = Op::Logs {
        container: q.get("container").cloned().filter(|s| !s.is_empty()),
        stack: q.get("stack").cloned().filter(|s| !s.is_empty()),
        containers: q
            .get("containers")
            .map(|c| c.split(',').filter(|s| !s.is_empty()).take(64).map(String::from).collect())
            .unwrap_or_default(),
        tail: q
            .get("tail")
            .and_then(|t| t.parse().ok())
            .unwrap_or(200)
            .min(5000),
        follow: q.get("follow").map(|f| f != "0").unwrap_or(true),
    };
    let handle = agent_stream(cx, op, true).await?;
    Ok(Sse::new(sse_lines(handle)).keep_alive(KeepAlive::new()))
}
