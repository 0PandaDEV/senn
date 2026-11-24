use std::{
    collections::{HashMap, VecDeque},
    fmt::Write,
    sync::{Arc, LazyLock, Mutex, OnceLock},
    time::{Duration, Instant},
};

use serde_json::{Map, Value, json};
use tokio::sync::{broadcast, mpsc};

use super::{
    config::config,
    hub::{self, Stats},
    state,
};

const RETAIN: Duration = Duration::from_secs(300);
const BATCH_BYTES: usize = 8 * 1024 * 1024;
const MAX_POINTS: f64 = 4000.0;
const MAX_SERIES: usize = 200;

const METRICS: &[&str] = &[
    "senn_host_cpu_user_percent",
    "senn_host_cpu_system_percent",
    "senn_host_cpu_iowait_percent",
    "senn_host_cpu_steal_percent",
    "senn_host_cpu_irq_percent",
    "senn_host_load5",
    "senn_host_load15",
    "senn_host_mem_free_bytes",
    "senn_host_mem_cached_bytes",
    "senn_host_mem_buffers_bytes",
    "senn_host_mem_available_bytes",
    "senn_host_swap_total_bytes",
    "senn_host_swap_in_bytes_per_second",
    "senn_host_swap_out_bytes_per_second",
    "senn_host_io_read_ops_per_second",
    "senn_host_io_write_ops_per_second",
    "senn_host_io_busy_percent",
    "senn_host_net_rx_packets_per_second",
    "senn_host_net_tx_packets_per_second",
    "senn_host_net_rx_errors_per_second",
    "senn_host_net_tx_errors_per_second",
    "senn_host_context_switches_per_second",
    "senn_host_interrupts_per_second",
    "senn_host_processes_running",
    "senn_host_processes_blocked",
    "senn_host_processes_total",
    "senn_host_tcp_connections",
    "senn_host_open_files",
    "senn_host_pressure_cpu_percent",
    "senn_host_pressure_memory_percent",
    "senn_host_pressure_memory_full_percent",
    "senn_host_pressure_io_percent",
    "senn_host_pressure_io_full_percent",
    "senn_host_temperature_celsius",
    "senn_host_uptime_seconds",
    "senn_host_docker_images",
    "senn_host_docker_images_bytes",
    "senn_host_docker_volumes",
    "senn_host_docker_networks",
    "senn_container_pids",
    "senn_container_mem_limit_bytes",
    "senn_container_mem_percent",
    "senn_container_net_rx_packets_per_second",
    "senn_container_net_tx_packets_per_second",
    "senn_container_cpu_throttled_per_second",
    "senn_container_io_read_ops_per_second",
    "senn_container_io_write_ops_per_second",
    "senn_container_healthy",
    "senn_container_restarting",
    "senn_host_cpu_percent",
    "senn_host_load1",
    "senn_host_mem_used_bytes",
    "senn_host_mem_total_bytes",
    "senn_host_swap_used_bytes",
    "senn_host_disk_used_bytes",
    "senn_host_disk_total_bytes",
    "senn_host_net_rx_bytes_per_second",
    "senn_host_net_tx_bytes_per_second",
    "senn_host_io_read_bytes_per_second",
    "senn_host_io_write_bytes_per_second",
    "senn_container_cpu_percent",
    "senn_container_mem_bytes",
    "senn_container_net_rx_bytes_per_second",
    "senn_container_net_tx_bytes_per_second",
    "senn_container_io_read_bytes_per_second",
    "senn_container_io_write_bytes_per_second",
    "senn_container_running",
];

pub struct Tick {
    pub agent: String,
    pub ts: i64,
    pub host: Vec<(&'static str, f64)>,
    pub containers: Vec<ContainerTick>,
}

pub struct ContainerTick {
    pub name: String,
    pub stack: Option<String>,
    pub service: Option<String>,
    pub values: Vec<(&'static str, f64)>,
}

impl Tick {
    pub fn to_json(&self) -> Value {
        let values = |v: &[(&'static str, f64)]| -> Map<String, Value> {
            v.iter().map(|(k, x)| ((*k).to_string(), json!(x))).collect()
        };
        json!({
            "t": "sample",
            "agent": self.agent,
            "ts": self.ts,
            "host": values(&self.host),
            "containers": self.containers.iter().map(|c| json!({
                "container": c.name,
                "stack": c.stack,
                "service": c.service,
                "values": values(&c.values),
            })).collect::<Vec<_>>(),
        })
    }
}

struct Raw {
    agent: String,
    at_ms: i64,
    stats: Stats,
}

#[derive(Clone)]
struct Known {
    name: String,
    stack: Option<String>,
    service: Option<String>,
    running: bool,
    restarting: bool,
    healthy: Option<bool>,
}

#[derive(Default)]
struct Entry {
    known: HashMap<String, Known>,
    at: Option<Instant>,
    tried: Option<Instant>,
    busy: bool,
    stale: bool,
    last_ts: i64,
}

static TX: OnceLock<mpsc::UnboundedSender<Raw>> = OnceLock::new();
static LABELS: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(Default::default);
static SAMPLES: LazyLock<broadcast::Sender<Arc<Tick>>> = LazyLock::new(|| broadcast::channel(64).0);
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .expect("http client")
});

pub fn enabled() -> bool {
    config().metrics_url.is_some()
}

pub fn subscribe() -> broadcast::Receiver<Arc<Tick>> {
    SAMPLES.subscribe()
}

pub fn start() {
    let Some(url) = config().metrics_url.clone() else {
        tracing::info!("metrics storage disabled");
        return;
    };
    let (tx, rx) = mpsc::unbounded_channel();
    if TX.set(tx).is_ok() {
        tokio::spawn(run(url, rx));
    }
}

pub fn ingest(agent: &str, stats: Stats) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(Raw {
            agent: agent.to_string(),
            at_ms: jiff::Timestamp::now().as_millisecond(),
            stats,
        });
    }
}

fn last_sample(agent: &str) -> Option<f64> {
    let labels = LABELS.lock().expect("labels lock");
    labels.get(agent).map(|e| e.last_ts).filter(|t| *t > 0).map(|t| t as f64 / 1000.0)
}

pub fn invalidate(agent: &str) {
    if let Some(entry) = LABELS.lock().expect("labels lock").get_mut(agent) {
        entry.stale = true;
    }
}

async fn refresh(agent: String) {
    let result = hub::host_state(&agent).await;
    let mut labels = LABELS.lock().expect("labels lock");
    let Some(entry) = labels.get_mut(&agent) else { return };
    entry.busy = false;
    if let Ok(state) = result {
        entry.known = state
            .containers
            .into_iter()
            .map(|c| {
                (
                    c.id,
                    Known {
                        name: c.name,
                        stack: c.stack,
                        service: c.service,
                        running: c.state == "running",
                        restarting: c.state == "restarting",
                        healthy: match c.health.as_deref() {
                            Some("healthy") => Some(true),
                            Some("unhealthy") => Some(false),
                            _ => None,
                        },
                    },
                )
            })
            .collect();
        entry.at = Some(Instant::now());
        entry.stale = false;
    }
}

fn round2(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

fn build(raw: Raw) -> Option<Tick> {
    let mut labels = LABELS.lock().expect("labels lock");
    let entry = labels.entry(raw.agent.clone()).or_default();
    let mut ts = (raw.at_ms + 500) / 1000 * 1000;
    if ts <= entry.last_ts {
        ts = entry.last_ts + 1000;
    }
    if ts > raw.at_ms + 5000 {
        return None;
    }
    entry.last_ts = ts;
    let unknown = raw.stats.containers.keys().any(|id| !entry.known.contains_key(id));
    let due = entry.at.is_none_or(|t| t.elapsed() > Duration::from_secs(30))
        || entry.stale
        || (unknown && entry.tried.is_none_or(|t| t.elapsed() > Duration::from_secs(5)));
    if due && !entry.busy {
        entry.busy = true;
        entry.tried = Some(Instant::now());
        tokio::spawn(refresh(raw.agent.clone()));
    }
    let mut host = Vec::new();
    if let Some(h) = &raw.stats.host {
        host.extend([
            ("senn_host_cpu_percent", round2(h.cpu)),
            ("senn_host_load1", round2(h.load[0])),
            ("senn_host_mem_used_bytes", h.mem_used as f64),
            ("senn_host_mem_total_bytes", h.mem_total as f64),
            ("senn_host_swap_used_bytes", h.swap_used as f64),
            ("senn_host_disk_used_bytes", h.disk_used as f64),
            ("senn_host_disk_total_bytes", h.disk_total as f64),
            ("senn_host_net_rx_bytes_per_second", h.net_rx as f64),
            ("senn_host_net_tx_bytes_per_second", h.net_tx as f64),
            ("senn_host_io_read_bytes_per_second", h.io_read as f64),
            ("senn_host_io_write_bytes_per_second", h.io_write as f64),
            ("senn_host_load5", round2(h.load[1])),
            ("senn_host_load15", round2(h.load[2])),
            ("senn_host_swap_total_bytes", h.swap_total as f64),
            ("senn_host_uptime_seconds", h.uptime as f64),
        ]);
        if let Some(d) = &h.docker {
            host.extend([
                ("senn_host_docker_images", d.images as f64),
                ("senn_host_docker_images_bytes", d.images_size as f64),
                ("senn_host_docker_volumes", d.volumes as f64),
                ("senn_host_docker_networks", d.networks as f64),
            ]);
        }
        if let Some(x) = &h.extra {
            host.extend([
                ("senn_host_cpu_user_percent", round2(x.cpu_user)),
                ("senn_host_cpu_system_percent", round2(x.cpu_system)),
                ("senn_host_cpu_iowait_percent", round2(x.cpu_iowait)),
                ("senn_host_cpu_steal_percent", round2(x.cpu_steal)),
                ("senn_host_cpu_irq_percent", round2(x.cpu_irq)),
                ("senn_host_mem_free_bytes", x.mem_free as f64),
                ("senn_host_mem_cached_bytes", x.mem_cached as f64),
                ("senn_host_mem_buffers_bytes", x.mem_buffers as f64),
                ("senn_host_mem_available_bytes", x.mem_available as f64),
                ("senn_host_swap_in_bytes_per_second", x.swap_in as f64),
                ("senn_host_swap_out_bytes_per_second", x.swap_out as f64),
                ("senn_host_io_read_ops_per_second", x.io_read_ops as f64),
                ("senn_host_io_write_ops_per_second", x.io_write_ops as f64),
                ("senn_host_io_busy_percent", round2(x.io_busy)),
                ("senn_host_net_rx_packets_per_second", x.net_rx_packets as f64),
                ("senn_host_net_tx_packets_per_second", x.net_tx_packets as f64),
                ("senn_host_net_rx_errors_per_second", x.net_rx_errors as f64),
                ("senn_host_net_tx_errors_per_second", x.net_tx_errors as f64),
                ("senn_host_context_switches_per_second", x.ctx_switches as f64),
                ("senn_host_interrupts_per_second", x.interrupts as f64),
                ("senn_host_processes_running", x.procs_running as f64),
                ("senn_host_processes_blocked", x.procs_blocked as f64),
                ("senn_host_processes_total", x.procs_total as f64),
                ("senn_host_tcp_connections", x.tcp_inuse as f64),
                ("senn_host_open_files", x.open_files as f64),
            ]);
            for (name, value) in [
                ("senn_host_pressure_cpu_percent", x.pressure_cpu),
                ("senn_host_pressure_memory_percent", x.pressure_mem),
                ("senn_host_pressure_memory_full_percent", x.pressure_mem_full),
                ("senn_host_pressure_io_percent", x.pressure_io),
                ("senn_host_pressure_io_full_percent", x.pressure_io_full),
                ("senn_host_temperature_celsius", x.temp),
            ] {
                if let Some(v) = value {
                    host.push((name, round2(v)));
                }
            }
        }
    }
    let containers = entry
        .known
        .iter()
        .map(|(id, k)| {
            let mut values = vec![
                ("senn_container_running", if k.running { 1.0 } else { 0.0 }),
                ("senn_container_restarting", if k.restarting { 1.0 } else { 0.0 }),
            ];
            if let Some(h) = k.healthy {
                values.push(("senn_container_healthy", if h { 1.0 } else { 0.0 }));
            }
            if let Some(s) = raw.stats.containers.get(id) {
                if s.limit > 0 {
                    values.push(("senn_container_mem_limit_bytes", s.limit as f64));
                    values.push(("senn_container_mem_percent", round2(s.mem as f32 / s.limit as f32 * 100.0)));
                }
                for (name, v) in [
                    ("senn_container_pids", s.pids),
                    ("senn_container_net_rx_packets_per_second", s.net_rx_packets),
                    ("senn_container_net_tx_packets_per_second", s.net_tx_packets),
                    ("senn_container_cpu_throttled_per_second", s.throttled),
                    ("senn_container_io_read_ops_per_second", s.io_read_ops),
                    ("senn_container_io_write_ops_per_second", s.io_write_ops),
                ] {
                    if let Some(v) = v {
                        values.push((name, v as f64));
                    }
                }
                values.extend([
                    ("senn_container_cpu_percent", round2(s.cpu)),
                    ("senn_container_mem_bytes", s.mem as f64),
                    ("senn_container_net_rx_bytes_per_second", s.net_rx as f64),
                    ("senn_container_net_tx_bytes_per_second", s.net_tx as f64),
                    ("senn_container_io_read_bytes_per_second", s.io_read as f64),
                    ("senn_container_io_write_bytes_per_second", s.io_write as f64),
                ]);
            }
            ContainerTick {
                name: k.name.clone(),
                stack: k.stack.clone(),
                service: k.service.clone(),
                values,
            }
        })
        .collect();
    Some(Tick {
        agent: raw.agent,
        ts,
        host,
        containers,
    })
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn render(t: &Tick) -> String {
    let host = escape(&t.agent);
    let mut out = String::new();
    for (name, v) in t.host.iter().filter(|(_, v)| v.is_finite()) {
        let _ = writeln!(out, "{name}{{host=\"{host}\"}} {} {}", number(*v), t.ts);
    }
    for c in &t.containers {
        let mut labels = format!("host=\"{host}\",container=\"{}\"", escape(&c.name));
        if let Some(s) = &c.stack {
            let _ = write!(labels, ",stack=\"{}\"", escape(s));
        }
        if let Some(s) = &c.service {
            let _ = write!(labels, ",service=\"{}\"", escape(s));
        }
        for (name, v) in c.values.iter().filter(|(_, v)| v.is_finite()) {
            let _ = writeln!(out, "{name}{{{labels}}} {} {}", number(*v), t.ts);
        }
    }
    out
}

async fn run(url: String, mut rx: mpsc::UnboundedReceiver<Raw>) {
    let endpoint = format!("{url}/api/v1/import/prometheus");
    let mut queue: VecDeque<(Instant, String)> = VecDeque::new();
    let mut flush = tokio::time::interval(Duration::from_secs(1));
    flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut failing = false;
    let mut dropped_total: usize = 0;
    let mut warned = Instant::now();
    tracing::info!("metrics storage at {url}");
    loop {
        tokio::select! {
            raw = rx.recv() => {
                let Some(raw) = raw else { break };
                if let Some(tick) = build(raw) {
                    queue.push_back((Instant::now(), render(&tick)));
                    let _ = SAMPLES.send(Arc::new(tick));
                }
            }
            _ = flush.tick() => {
                let mut dropped = 0;
                while queue.front().is_some_and(|(at, _)| at.elapsed() > RETAIN) {
                    queue.pop_front();
                    dropped += 1;
                }
                dropped_total += dropped;
                if queue.is_empty() {
                    continue;
                }
                let mut body = String::new();
                let mut taken = 0;
                for (_, part) in &queue {
                    if !body.is_empty() && body.len() + part.len() > BATCH_BYTES {
                        break;
                    }
                    body.push_str(part);
                    taken += 1;
                }
                let outcome = match CLIENT.post(&endpoint).body(body).send().await {
                    Ok(r) if r.status().is_success() => Ok(()),
                    Ok(r) => Err(format!("status {}", r.status())),
                    Err(e) => Err(e.to_string()),
                };
                match outcome {
                    Ok(()) => {
                        queue.drain(..taken);
                        if failing {
                            failing = false;
                            tracing::info!("metrics storage reachable again, dropped {dropped_total} samples while away");
                            dropped_total = 0;
                        }
                    }
                    Err(e) => {
                        if !failing || warned.elapsed() > Duration::from_secs(60) {
                            failing = true;
                            warned = Instant::now();
                            tracing::warn!("metrics storage at {url} unreachable ({e}), buffering up to 5 minutes, {dropped_total} samples dropped so far");
                        }
                    }
                }
            }
        }
    }
}

pub struct Query {
    pub host: String,
    pub metric: String,
    pub from: f64,
    pub to: f64,
    pub step: f64,
    pub container: Option<String>,
    pub stack: Option<String>,
    pub service: Option<String>,
    pub by: Option<String>,
}

#[derive(Clone, Copy)]
enum Kind {
    Raw,
    Avg,
    Min,
    Max,
}

impl Kind {
    fn flip(self) -> Self {
        match self {
            Kind::Min => Kind::Max,
            Kind::Max => Kind::Min,
            other => other,
        }
    }
}

fn literal(value: &str) -> String {
    format!("\"{}\"", escape(value))
}

fn selector(q: &Query) -> String {
    let mut out = format!("{}{{host={}", q.metric, literal(&q.host));
    for (key, value) in [("container", &q.container), ("stack", &q.stack), ("service", &q.service)] {
        if let Some(v) = value {
            let _ = write!(out, ",{key}={}", literal(v));
        }
    }
    out.push('}');
    out
}

fn inner(q: &Query, step: u64, kind: Kind) -> String {
    let sel = selector(q);
    match kind {
        Kind::Raw => sel,
        Kind::Avg => format!("avg_over_time({sel}[{step}s])"),
        Kind::Min => format!("min_over_time({sel}[{step}s])"),
        Kind::Max => format!("max_over_time({sel}[{step}s])"),
    }
}

fn expression(q: &Query, step: u64, kind: Kind) -> String {
    match q.by.as_deref() {
        None => inner(q, step, kind),
        Some("sum") => format!("sum({})", inner(q, step, kind)),
        Some("count") => format!("count({})", inner(q, step, kind)),
        Some("stopped") => {
            let x = inner(q, step, kind.flip());
            format!("count({x}) - sum({x})")
        }
        Some(label) => format!("sum by ({label}) ({})", inner(q, step, kind)),
    }
}

type Rows = Vec<(Map<String, Value>, Vec<(f64, f64)>)>;

async fn range(url: &str, expr: String, from: f64, to: f64, step: u64) -> Result<Rows, (u16, String)> {
    let response = CLIENT
        .get(format!("{url}/api/v1/query_range"))
        .query(&[
            ("query", expr),
            ("start", format!("{from}")),
            ("end", format!("{to}")),
            ("step", format!("{step}s")),
            ("max_lookback", format!("{}s", step.max(2))),
        ])
        .send()
        .await
        .map_err(|_| (502, "Metrics storage is unreachable".to_string()))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|_| (502, "Metrics storage sent an invalid answer".to_string()))?;
    if !status.is_success() || body["status"] != "success" {
        let message = body["error"].as_str().unwrap_or("query failed");
        return Err((400, message.chars().take(300).collect()));
    }
    let mut rows = Rows::new();
    for item in body["data"]["result"].as_array().into_iter().flatten() {
        let mut labels = item["metric"].as_object().cloned().unwrap_or_default();
        labels.remove("__name__");
        let points = item["values"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let t = p.get(0)?.as_f64()?;
                let v = p.get(1)?.as_str()?.parse::<f64>().ok()?;
                v.is_finite().then_some((t, v))
            })
            .collect();
        rows.push((labels, points));
    }
    Ok(rows)
}

fn index(rows: Rows) -> HashMap<String, HashMap<i64, f64>> {
    rows.into_iter()
        .map(|(labels, points)| {
            let key = Value::Object(labels).to_string();
            let map = points.into_iter().map(|(t, v)| ((t * 1000.0).round() as i64, v)).collect();
            (key, map)
        })
        .collect()
}

pub async fn query(q: Query) -> Result<Value, (u16, String)> {
    let url = config().metrics_url.clone().ok_or((404, "Metrics storage is not configured".to_string()))?;
    if !METRICS.contains(&q.metric.as_str()) {
        return Err((400, "Unknown metric".into()));
    }
    if let Some(by) = q.by.as_deref()
        && !["sum", "count", "stopped", "container", "stack", "service"].contains(&by)
    {
        return Err((400, "Unknown grouping".into()));
    }
    if !q.from.is_finite() || !q.to.is_finite() || !q.step.is_finite() || q.to <= q.from {
        return Err((400, "Invalid time range".into()));
    }
    if q.to - q.from > 86400.0 * 800.0 {
        return Err((400, "Time range is too large".into()));
    }
    let step = q.step.max(1.0).ceil().max(((q.to - q.from) / MAX_POINTS).ceil()) as u64;
    let (from, to) = (q.from, q.to);
    let raw = step <= 1;
    let limit = last_sample(&q.host).map_or(to, |last| to.min(last + step as f64 - 0.001));
    let (avg, min, max) = if raw {
        let rows = range(&url, expression(&q, step, Kind::Raw), from, to, step).await?;
        (rows, None, None)
    } else {
        let (a, lo, hi) = futures::join!(
            range(&url, expression(&q, step, Kind::Avg), from, to, step),
            range(&url, expression(&q, step, Kind::Min), from, to, step),
            range(&url, expression(&q, step, Kind::Max), from, to, step),
        );
        (a?, Some(index(lo?)), Some(index(hi?)))
    };
    let series: Vec<Value> = avg
        .into_iter()
        .take(MAX_SERIES)
        .map(|(labels, points)| {
            let key = Value::Object(labels.clone()).to_string();
            let lows = min.as_ref().and_then(|m| m.get(&key));
            let highs = max.as_ref().and_then(|m| m.get(&key));
            let points: Vec<Value> = points
                .into_iter()
                .filter(|(t, _)| *t <= limit)
                .map(|(t, v)| {
                    let id = (t * 1000.0).round() as i64;
                    let lo = lows.and_then(|m| m.get(&id)).copied().unwrap_or(v).min(v);
                    let hi = highs.and_then(|m| m.get(&id)).copied().unwrap_or(v).max(v);
                    json!([t, v, lo, hi])
                })
                .collect();
            json!({ "labels": labels, "points": points })
        })
        .collect();
    Ok(json!({ "step": step, "from": from, "to": to, "series": series, "now": state::now() }))
}
