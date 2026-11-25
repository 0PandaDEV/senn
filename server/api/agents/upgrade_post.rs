use serde_json::Value;
use topcoat::{
    Result,
    context::Cx,
    router::{content::Json, response::Response, route},
};

use crate::{
    proto::Op,
    server::utils::{
        http_util::{agent_record, start_job},
        logging::plain,
        release,
    },
};

#[route(POST "/api/agents/{agent}/upgrade")]
pub async fn upgrade(cx: &Cx, Json(body): Json<Value>) -> Result<Response> {
    let record = agent_record(cx).await?;
    let want = body
        .get("version")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .unwrap_or(&record.config.channel)
        .to_string();
    let releases = release::list().await;
    let release = release::resolve(&releases, &want)
        .ok_or_else(|| plain(404, "No matching release found"))?;
    start_job(cx, Op::SelfUpdate { tag: release.tag }).await
}
