use std::collections::HashMap;

use bollard::{
    Docker,
    query_parameters::{
        EventsOptionsBuilder, KillContainerOptionsBuilder, ListContainersOptionsBuilder,
        LogsOptionsBuilder, RemoveContainerOptionsBuilder, RestartContainerOptionsBuilder,
        StatsOptionsBuilder, StopContainerOptionsBuilder,
    },
};
use futures::{Stream, StreamExt, future::join_all};

use crate::proto::{ContainerAction, ContainerInfo, DockerStats, PortInfo};

pub fn connect() -> Result<Docker, String> {
    Docker::connect_with_local_defaults().map_err(|e| e.to_string())
}

fn enum_text<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct Sample {
    pub cpu: f32,
    pub mem: u64,
    pub limit: u64,
    pub net_rx: u64,
    pub net_tx: u64,
    pub io_read: u64,
    pub io_write: u64,
    pub pids: u64,
    pub net_rx_packets: u64,
    pub net_tx_packets: u64,
    pub throttled: u64,
    pub io_read_ops: u64,
    pub io_write_ops: u64,
}

#[derive(Clone, Copy, Default)]
pub struct Counters {
    cpu: u64,
    rx: u64,
    tx: u64,
    read: u64,
    write: u64,
    rx_packets: u64,
    tx_packets: u64,
    throttled: u64,
    read_ops: u64,
    write_ops: u64,
}

pub type Prev = HashMap<String, (Counters, std::time::Instant)>;

pub fn rate(now: u64, before: u64, secs: f64) -> u64 {
    if now < before { 0 } else { ((now - before) as f64 / secs) as u64 }
}

fn rss_fallback(id: &str) -> u64 {
    let paths = [
        format!("/sys/fs/cgroup/system.slice/docker-{id}.scope/cgroup.procs"),
        format!("/sys/fs/cgroup/docker/{id}/cgroup.procs"),
        format!("/sys/fs/cgroup/cpu/docker/{id}/cgroup.procs"),
    ];
    let Some(procs) = paths.iter().find_map(|p| std::fs::read_to_string(p).ok()) else {
        return 0;
    };
    procs
        .lines()
        .filter_map(|pid| std::fs::read_to_string(format!("/proc/{}/status", pid.trim())).ok())
        .filter_map(|status| {
            status
                .lines()
                .find_map(|l| l.strip_prefix("VmRSS:"))
                .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
        })
        .sum::<u64>()
        * 1024
}

async fn sample_one(docker: &Docker, id: String) -> Result<(String, Counters, u64, u64, u64), String> {
    let options = StatsOptionsBuilder::default().stream(false).one_shot(true).build();
    let mut stream = std::pin::pin!(docker.stats(&id, Some(options)));
    let stats = tokio::time::timeout(std::time::Duration::from_secs(8), stream.next())
        .await
        .map_err(|_| "stats timed out".to_string())?
        .ok_or_else(|| "empty stats".to_string())?
        .map_err(|e| e.to_string())?;
    let cpu = stats
        .cpu_stats
        .as_ref()
        .and_then(|c| c.cpu_usage.as_ref())
        .and_then(|u| u.total_usage)
        .unwrap_or(0);
    let memory = stats.memory_stats.as_ref();
    let cache = memory
        .and_then(|m| m.stats.as_ref())
        .and_then(|s| s.get("inactive_file").or_else(|| s.get("total_inactive_file")))
        .copied()
        .unwrap_or(0);
    let mut mem = memory.and_then(|m| m.usage).unwrap_or(0).saturating_sub(cache);
    if mem == 0 {
        mem = rss_fallback(&id);
    }
    let limit = memory.and_then(|m| m.limit).unwrap_or(0);
    let (rx, tx) = stats
        .networks
        .as_ref()
        .map(|n| n.values().fold((0, 0), |(r, t), v| (r + v.rx_bytes.unwrap_or(0), t + v.tx_bytes.unwrap_or(0))))
        .unwrap_or((0, 0));
    let (rx_packets, tx_packets) = stats
        .networks
        .as_ref()
        .map(|n| n.values().fold((0, 0), |(r, t), v| (r + v.rx_packets.unwrap_or(0), t + v.tx_packets.unwrap_or(0))))
        .unwrap_or((0, 0));
    let throttled = stats
        .cpu_stats
        .as_ref()
        .and_then(|c| c.throttling_data.as_ref())
        .and_then(|t| t.throttled_periods)
        .unwrap_or(0);
    let pids = stats.pids_stats.as_ref().and_then(|p| p.current).unwrap_or(0);
    let (read_ops, write_ops) = stats
        .blkio_stats
        .as_ref()
        .and_then(|b| b.io_serviced_recursive.as_ref())
        .map(|entries| {
            entries.iter().fold((0, 0), |(r, w), e| match e.op.as_deref().map(str::to_lowercase).as_deref() {
                Some("read") => (r + e.value.unwrap_or(0), w),
                Some("write") => (r, w + e.value.unwrap_or(0)),
                _ => (r, w),
            })
        })
        .unwrap_or((0, 0));
    let (read, write) = stats
        .blkio_stats
        .as_ref()
        .and_then(|b| b.io_service_bytes_recursive.as_ref())
        .map(|entries| {
            entries.iter().fold((0, 0), |(r, w), e| match e.op.as_deref().map(str::to_lowercase).as_deref() {
                Some("read") => (r + e.value.unwrap_or(0), w),
                Some("write") => (r, w + e.value.unwrap_or(0)),
                _ => (r, w),
            })
        })
        .unwrap_or((0, 0));
    Ok((id, Counters { cpu, rx, tx, read, write, rx_packets, tx_packets, throttled, read_ops, write_ops }, mem, limit, pids))
}

pub async fn sample(docker: &Docker, prev: &mut Prev) -> (HashMap<String, Sample>, Vec<String>) {
    let filters = HashMap::from([("status", vec!["running"])]);
    let options = ListContainersOptionsBuilder::default().filters(&filters).build();
    let list = match docker.list_containers(Some(options)).await {
        Ok(l) => l,
        Err(e) => return (HashMap::new(), vec![e.to_string()]),
    };
    let ids = list.into_iter().filter_map(|c| c.id);
    let results = join_all(ids.map(|id| sample_one(docker, id))).await;
    let now = std::time::Instant::now();
    let mut out = HashMap::new();
    let mut errors = Vec::new();
    let mut next = Prev::new();
    for r in results {
        match r {
            Ok((id, c, mem, limit, pids)) => {
                let mut sample = Sample { mem, limit, pids, ..Default::default() };
                if let Some((b, at)) = prev.get(&id) {
                    let secs = now.duration_since(*at).as_secs_f64().max(0.001);
                    sample.cpu = (rate(c.cpu, b.cpu, secs) as f64 / 1e9 * 100.0) as f32;
                    sample.net_rx = rate(c.rx, b.rx, secs);
                    sample.net_tx = rate(c.tx, b.tx, secs);
                    sample.io_read = rate(c.read, b.read, secs);
                    sample.io_write = rate(c.write, b.write, secs);
                    sample.net_rx_packets = rate(c.rx_packets, b.rx_packets, secs);
                    sample.net_tx_packets = rate(c.tx_packets, b.tx_packets, secs);
                    sample.throttled = rate(c.throttled, b.throttled, secs);
                    sample.io_read_ops = rate(c.read_ops, b.read_ops, secs);
                    sample.io_write_ops = rate(c.write_ops, b.write_ops, secs);
                }
                next.insert(id.clone(), (c, now));
                out.insert(id, sample);
            }
            Err(e) => errors.push(e),
        }
    }
    *prev = next;
    (out, errors)
}

pub async fn containers(
    docker: &Docker,
    stack_dirs: &HashMap<String, String>,
    samples: &HashMap<String, Sample>,
) -> Result<Vec<ContainerInfo>, String> {
    let options = ListContainersOptionsBuilder::default().all(true).build();
    let list = docker
        .list_containers(Some(options))
        .await
        .map_err(|e| e.to_string())?;
    let mut out: Vec<ContainerInfo> = list
        .into_iter()
        .map(|c| {
            let labels = c.labels.unwrap_or_default();
            let working_dir = labels
                .get("com.docker.compose.project.working_dir")
                .cloned();
            let project = labels.get("com.docker.compose.project").cloned();
            let stack = working_dir
                .as_ref()
                .and_then(|d| stack_dirs.get(d.trim_end_matches('/')).cloned())
                .or_else(|| {
                    project
                        .as_ref()
                        .filter(|_| working_dir.is_none())
                        .and_then(|p| {
                            stack_dirs
                                .values()
                                .find(|name| name.to_lowercase() == *p)
                                .cloned()
                        })
                });
            let mut ports: Vec<PortInfo> = c
                .ports
                .unwrap_or_default()
                .into_iter()
                .map(|p| PortInfo {
                    private: p.private_port,
                    public: p.public_port,
                    proto: p
                        .typ
                        .as_ref()
                        .map(enum_text)
                        .unwrap_or_else(|| "tcp".into()),
                    ip: p.ip,
                })
                .collect();
            ports.sort_by_key(|p| (p.private, p.public));
            ports.dedup_by(|a, b| {
                a.private == b.private && a.public == b.public && a.proto == b.proto
            });
            let status = c.status.unwrap_or_default();
            let health = ["(healthy)", "(unhealthy)", "(health: starting)"]
                .iter()
                .find(|h| status.contains(*h))
                .map(|h| {
                    h.trim_matches(|c| c == '(' || c == ')')
                        .replace("health: ", "")
                });
            let id = c.id.unwrap_or_default();
            let sample = samples.get(&id);
            ContainerInfo {
                cpu: sample.map(|s| s.cpu),
                mem: sample.map(|s| s.mem),
                mem_limit: sample.map(|s| s.limit).filter(|l| *l > 0),
                net_rx: sample.map(|s| s.net_rx),
                net_tx: sample.map(|s| s.net_tx),
                io_read: sample.map(|s| s.io_read),
                io_write: sample.map(|s| s.io_write),
                id,
                name: c
                    .names
                    .and_then(|n| n.into_iter().next())
                    .map(|n| n.trim_start_matches('/').to_string())
                    .unwrap_or_default(),
                image: c.image.unwrap_or_default(),
                image_id: c.image_id.unwrap_or_default(),
                state: c.state.as_ref().map(enum_text).unwrap_or_default(),
                status,
                health,
                created: c.created.unwrap_or(0),
                ports,
                stack,
                service: labels.get("com.docker.compose.service").cloned(),
                update_policy: labels
                    .get("senn.update")
                    .map(|v| v.to_lowercase())
                    .filter(|v| matches!(v.as_str(), "auto" | "notify" | "off"))
                    .unwrap_or_default(),
            }
        })
        .collect();
    out.sort_by(|a, b| (&a.stack, &a.service, &a.name).cmp(&(&b.stack, &b.service, &b.name)));
    Ok(out)
}

pub async fn container_action(
    docker: &Docker,
    id: &str,
    action: ContainerAction,
) -> Result<(), String> {
    let err = |e: bollard::errors::Error| e.to_string();
    match action {
        ContainerAction::Start => docker.start_container(id, None).await.map_err(err),
        ContainerAction::Stop => docker
            .stop_container(
                id,
                Some(StopContainerOptionsBuilder::default().t(20).build()),
            )
            .await
            .map_err(err),
        ContainerAction::Restart => docker
            .restart_container(
                id,
                Some(RestartContainerOptionsBuilder::default().t(20).build()),
            )
            .await
            .map_err(err),
        ContainerAction::Kill => docker
            .kill_container(id, Some(KillContainerOptionsBuilder::default().build()))
            .await
            .map_err(err),
        ContainerAction::Remove => docker
            .remove_container(
                id,
                Some(RemoveContainerOptionsBuilder::default().force(true).build()),
            )
            .await
            .map_err(err),
    }
}

pub fn logs(
    docker: &Docker,
    id: &str,
    tail: u32,
    follow: bool,
) -> impl Stream<Item = Result<String, String>> + use<> {
    let options = LogsOptionsBuilder::default()
        .stdout(true)
        .stderr(true)
        .follow(follow)
        .timestamps(true)
        .tail(&tail.to_string())
        .build();
    docker
        .logs(id, Some(options))
        .map(|r| r.map(|out| out.to_string()).map_err(|e| e.to_string()))
}

pub struct Event {
    pub action: String,
    pub id: String,
    pub attrs: HashMap<String, String>,
}

pub fn events(docker: &Docker) -> impl Stream<Item = Event> + use<> {
    let filters = HashMap::from([("type", vec!["container", "image"])]);
    let options = EventsOptionsBuilder::default().filters(&filters).build();
    docker.events(Some(options)).filter_map(|e| async move {
        let e = e.ok()?;
        let action = e.action.unwrap_or_default();
        let actor = e.actor.unwrap_or_default();
        Some(Event {
            action,
            id: actor.id.unwrap_or_default(),
            attrs: actor.attributes.unwrap_or_default(),
        })
    })
}

pub fn notable(e: &Event) -> bool {
    match e.action.as_str() {
        "oom" => true,
        "die" => !matches!(e.attrs.get("exitCode").map(String::as_str), Some("0" | "137" | "143") | None),
        a => a.starts_with("health_status: unhealthy"),
    }
}

pub async fn docker_stats(docker: &Docker) -> Option<DockerStats> {
    let info = docker.info().await.ok()?;
    let images = docker
        .list_images(None::<bollard::query_parameters::ListImagesOptions>)
        .await
        .unwrap_or_default();
    let volumes = docker
        .list_volumes(None::<bollard::query_parameters::ListVolumesOptions>)
        .await
        .ok()
        .and_then(|v| v.volumes)
        .map(|v| v.len())
        .unwrap_or(0);
    let networks = docker
        .list_networks(None::<bollard::query_parameters::ListNetworksOptions>)
        .await
        .map(|n| n.len())
        .unwrap_or(0);
    Some(DockerStats {
        images: images.len() as u64,
        images_size: images.iter().map(|i| i.size.max(0) as u64).sum(),
        volumes: volumes as u64,
        networks: networks as u64,
        driver: info.driver.unwrap_or_default(),
        root_dir: info.docker_root_dir.unwrap_or_default(),
    })
}
