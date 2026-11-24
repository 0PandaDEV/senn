use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::{
    proto::Op,
    server::utils::{hub, jobs, state},
};

const TTL: Duration = Duration::from_secs(15 * 60);
const RETRY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone)]
pub struct Release {
    pub tag: String,
    pub nightly: bool,
}

static CACHE: Mutex<Option<(Instant, Vec<Release>)>> = Mutex::new(None);
static ATTEMPTS: LazyLock<Mutex<HashMap<String, (String, Instant)>>> =
    LazyLock::new(Default::default);

fn repo() -> String {
    std::env::var("SENN_REPO").unwrap_or_else(|_| "0PandaDEV/senn".into())
}

pub async fn list() -> Vec<Release> {
    if let Some((at, list)) = CACHE.lock().expect("release lock").as_ref()
        && at.elapsed() < TTL
    {
        return list.clone();
    }
    match fetch().await {
        Ok(list) => {
            *CACHE.lock().expect("release lock") = Some((Instant::now(), list.clone()));
            list
        }
        Err(e) => {
            tracing::warn!("release lookup failed: {e}");
            let mut cache = CACHE.lock().expect("release lock");
            let old = cache.as_ref().map(|(_, l)| l.clone()).unwrap_or_default();
            *cache = Some((Instant::now() - TTL + Duration::from_secs(60), old.clone()));
            old
        }
    }
}

async fn fetch() -> Result<Vec<Release>, String> {
    let client = reqwest::Client::builder()
        .user_agent("senn")
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let rows: Vec<Value> = client
        .get(format!(
            "https://api.github.com/repos/{}/releases?per_page=100",
            repo()
        ))
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?.to_string();
            Some(Release {
                nightly: tag.contains("-nightly"),
                tag,
            })
        })
        .collect())
}

pub fn resolve(list: &[Release], channel: &str) -> Option<String> {
    match channel {
        "nightly" => list.iter().find(|r| r.nightly),
        "stable" | "" => list.iter().find(|r| !r.tag.contains('-')),
        tag => list.iter().find(|r| r.tag == tag),
    }
    .map(|r| r.tag.clone())
}

pub fn version_of(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

fn parts(version: &str) -> (Vec<u64>, Option<Vec<u64>>) {
    let version = version.strip_prefix('v').unwrap_or(version);
    let (core, pre) = match version.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (version, None),
    };
    let nums = |s: &str| {
        s.split('.')
            .filter_map(|p| p.parse().ok())
            .collect::<Vec<u64>>()
    };
    (nums(core), pre.map(nums))
}

pub fn newer(candidate: &str, current: &str) -> bool {
    let (a, ap) = parts(candidate);
    let (b, bp) = parts(current);
    match a.cmp(&b) {
        std::cmp::Ordering::Equal => match (ap, bp) {
            (None, Some(_)) => true,
            (Some(x), Some(y)) => x > y,
            _ => false,
        },
        order => order.is_gt(),
    }
}

pub fn label(channel: &str) -> String {
    match channel {
        "nightly" => "latest nightly".into(),
        "stable" | "" => "latest release".into(),
        tag => format!("pinned to {tag}"),
    }
}

pub async fn check(agent_id: &str) {
    let Some(record) = state::agent(agent_id).await else {
        return;
    };
    if !record.config.self_update || !hub::online(agent_id) {
        return;
    }
    let Some(info) = record.info else {
        return;
    };
    if info.os != "linux" {
        return;
    }
    let releases = list().await;
    let Some(tag) = resolve(&releases, &record.config.channel) else {
        return;
    };
    let pinned = !matches!(record.config.channel.as_str(), "stable" | "nightly" | "");
    if version_of(&tag) == info.version || (!pinned && !newer(&tag, &info.version)) {
        return;
    }
    {
        let mut attempts = ATTEMPTS.lock().expect("attempts lock");
        if let Some((last, at)) = attempts.get(agent_id)
            && *last == tag
            && at.elapsed() < RETRY
        {
            return;
        }
        attempts.insert(agent_id.to_string(), (tag.clone(), Instant::now()));
    }
    match hub::open_stream(agent_id, Op::SelfUpdate { tag: tag.clone() }, false) {
        Ok(handle) => {
            tracing::info!("updating agent {} to {tag}", record.name);
            jobs::start(
                format!("Update agent {} to {tag}", record.name),
                format!("/api/agents/{agent_id}/upgrade"),
                handle,
            );
        }
        Err(e) => tracing::warn!("agent update for {} failed: {e}", record.name),
    }
}

pub fn later(agent_id: String, secs: u64) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        check(&agent_id).await;
    });
}

pub fn start() {
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            for agent in state::agents().await {
                check(&agent.id).await;
            }
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn ordering() {
        assert!(newer("v0.1.2", "0.1.1"));
        assert!(!newer("v0.1.1", "0.1.2"));
        assert!(newer("v0.1.2", "0.1.2-nightly.20261124.3"));
        assert!(!newer("v0.1.2-nightly.20261124.3", "0.1.2"));
        assert!(newer(
            "v0.1.2-nightly.20261125.4",
            "0.1.2-nightly.20261124.3"
        ));
        assert!(newer("v0.1.3-nightly.20261125.4", "0.1.2"));
        assert!(!newer("v0.1.2", "0.1.2"));
    }
}
