use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, oneshot};
use topcoat::router::content::websocket::{Message, WebSocket};

use crate::proto::{AgentConfig, FromAgent, HostState, Op, ToAgent};

use super::{
    metrics,
    state::{self, AgentInfo},
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct HubEvent {
    pub agent: String,
    pub kind: String,
    pub data: Value,
}

pub enum StreamMsg {
    Chunk(String),
    End(Result<(), String>),
}

enum Pending {
    Res(oneshot::Sender<Result<Value, String>>),
    Stream(mpsc::UnboundedSender<StreamMsg>),
}

struct Conn {
    session: u64,
    tx: mpsc::UnboundedSender<ToAgent>,
    pending: Mutex<HashMap<u64, Pending>>,
    next: AtomicU64,
    cache: Mutex<Option<(Instant, HostState)>>,
}

static CONNS: LazyLock<RwLock<HashMap<String, Arc<Conn>>>> = LazyLock::new(Default::default);
static SESSIONS: AtomicU64 = AtomicU64::new(1);
static EVENTS: LazyLock<broadcast::Sender<HubEvent>> = LazyLock::new(|| broadcast::channel(256).0);

pub fn subscribe() -> broadcast::Receiver<HubEvent> {
    EVENTS.subscribe()
}

fn emit(agent: &str, kind: &str, data: Value) {
    let _ = EVENTS.send(HubEvent {
        agent: agent.into(),
        kind: kind.into(),
        data,
    });
}

fn conn(id: &str) -> Option<Arc<Conn>> {
    CONNS.read().expect("hub lock").get(id).cloned()
}

pub fn online(id: &str) -> bool {
    conn(id).is_some()
}

pub fn push_config(id: &str, config: AgentConfig) {
    if let Some(c) = conn(id) {
        let _ = c.tx.send(ToAgent::Config { config });
        *c.cache.lock().expect("cache lock") = None;
    }
}

pub fn disconnect(id: &str) {
    CONNS.write().expect("hub lock").remove(id);
}

pub async fn request(id: &str, op: Op) -> Result<Value, String> {
    let result = request_inner(id, op).await;
    if let Some(c) = conn(id) {
        *c.cache.lock().expect("cache lock") = None;
    }
    result
}

async fn request_inner(id: &str, op: Op) -> Result<Value, String> {
    let c = conn(id).ok_or("agent is offline")?;
    let req_id = c.next.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = oneshot::channel();
    c.pending
        .lock()
        .expect("pending lock")
        .insert(req_id, Pending::Res(tx));
    if c.tx.send(ToAgent::Req { id: req_id, op }).is_err() {
        c.pending.lock().expect("pending lock").remove(&req_id);
        return Err("agent is offline".into());
    }
    match tokio::time::timeout(Duration::from_secs(60), rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("agent disconnected".into()),
        Err(_) => {
            c.pending.lock().expect("pending lock").remove(&req_id);
            Err("agent did not answer in time".into())
        }
    }
}

pub struct StreamHandle {
    pub rx: mpsc::UnboundedReceiver<StreamMsg>,
    conn: Arc<Conn>,
    id: u64,
    cancel_on_drop: bool,
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        self.conn
            .pending
            .lock()
            .expect("pending lock")
            .remove(&self.id);
        if self.cancel_on_drop {
            let _ = self.conn.tx.send(ToAgent::Cancel { id: self.id });
        }
    }
}

pub fn open_stream(id: &str, op: Op, cancel_on_drop: bool) -> Result<StreamHandle, String> {
    let c = conn(id).ok_or("agent is offline")?;
    let req_id = c.next.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::unbounded_channel();
    c.pending
        .lock()
        .expect("pending lock")
        .insert(req_id, Pending::Stream(tx));
    c.tx.send(ToAgent::Req { id: req_id, op })
        .map_err(|_| "agent is offline".to_string())?;
    Ok(StreamHandle {
        rx,
        conn: c,
        id: req_id,
        cancel_on_drop,
    })
}

pub async fn host_state(id: &str) -> Result<HostState, String> {
    let c = conn(id).ok_or("agent is offline")?;
    if let Some((at, state)) = c.cache.lock().expect("cache lock").as_ref()
        && at.elapsed() < Duration::from_secs(30)
    {
        return Ok(state.clone());
    }
    let value = request(id, Op::State).await?;
    let state: HostState = serde_json::from_value(value).map_err(|e| e.to_string())?;
    *c.cache.lock().expect("cache lock") = Some((Instant::now(), state.clone()));
    Ok(state)
}

#[derive(serde::Deserialize)]
pub struct Sample {
    pub cpu: f32,
    pub mem: u64,
    pub limit: u64,
    pub net_rx: u64,
    pub net_tx: u64,
    pub io_read: u64,
    pub io_write: u64,
    #[serde(default)]
    pub pids: Option<u64>,
    #[serde(default)]
    pub net_rx_packets: Option<u64>,
    #[serde(default)]
    pub net_tx_packets: Option<u64>,
    #[serde(default)]
    pub throttled: Option<u64>,
    #[serde(default)]
    pub io_read_ops: Option<u64>,
    #[serde(default)]
    pub io_write_ops: Option<u64>,
}

#[derive(serde::Deserialize)]
pub struct Stats {
    pub containers: HashMap<String, Sample>,
    pub host: Option<crate::proto::HostStats>,
}

fn patch_stats(conn: &Conn, stats: &Stats) {
    let mut cache = conn.cache.lock().expect("cache lock");
    let Some((_, state)) = cache.as_mut() else {
        return;
    };
    state.host = stats.host.clone();
    for c in &mut state.containers {
        match stats.containers.get(&c.id) {
            Some(s) => {
                c.cpu = Some(s.cpu);
                c.mem = Some(s.mem);
                c.mem_limit = Some(s.limit).filter(|l| *l > 0);
                c.net_rx = Some(s.net_rx);
                c.net_tx = Some(s.net_tx);
                c.io_read = Some(s.io_read);
                c.io_write = Some(s.io_write);
            }
            None => {
                c.cpu = None;
                c.mem = None;
                c.net_rx = None;
                c.net_tx = None;
                c.io_read = None;
                c.io_write = None;
            }
        }
    }
}

pub async fn serve(agent_id: String, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut outbox) = mpsc::unbounded_channel::<ToAgent>();
    let session = SESSIONS.fetch_add(1, Ordering::Relaxed);
    let connection = Arc::new(Conn {
        session,
        tx,
        pending: Mutex::new(HashMap::new()),
        next: AtomicU64::new(1),
        cache: Mutex::new(None),
    });

    let writer = tokio::spawn(async move {
        while let Some(msg) = outbox.recv().await {
            let Ok(text) = serde_json::to_string(&msg) else {
                continue;
            };
            if sink.send(Message::text(text)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let mut hello = false;
    loop {
        let next = tokio::time::timeout(Duration::from_secs(60), stream.next()).await;
        let frame = match next {
            Ok(Some(Ok(f))) => f,
            _ => break,
        };
        let Message::Text(text) = frame else {
            if matches!(frame, Message::Close(_)) {
                break;
            }
            continue;
        };
        let Ok(msg) = serde_json::from_str::<FromAgent>(&text) else {
            continue;
        };
        match msg {
            FromAgent::Hello {
                version,
                hostname,
                os,
                arch,
                docker,
                address,
            } => {
                let info = AgentInfo {
                    version,
                    hostname,
                    os,
                    arch,
                    docker,
                    address,
                };
                let Some(record) = state::set_agent_online(&agent_id, &info).await else {
                    break;
                };
                let old = CONNS
                    .write()
                    .expect("hub lock")
                    .insert(agent_id.clone(), connection.clone());
                drop(old);
                let _ = connection.tx.send(ToAgent::Welcome {
                    config: record.config,
                });
                hello = true;
                tracing::info!("agent {} ({}) connected", record.name, agent_id);
                emit(&agent_id, "online", Value::Null);
            }
            _ if !hello => break,
            FromAgent::Res {
                id,
                ok,
                data,
                error,
            } => {
                let pending = connection.pending.lock().expect("pending lock").remove(&id);
                match pending {
                    Some(Pending::Res(tx)) => {
                        let _ = tx.send(if ok {
                            Ok(data)
                        } else {
                            Err(error.unwrap_or_else(|| "failed".into()))
                        });
                    }
                    Some(Pending::Stream(tx)) => {
                        let _ = tx.send(StreamMsg::End(if ok {
                            Ok(())
                        } else {
                            Err(error.unwrap_or_else(|| "failed".into()))
                        }));
                    }
                    None => {}
                }
            }
            FromAgent::Chunk { id, data } => {
                if let Some(Pending::Stream(tx)) =
                    connection.pending.lock().expect("pending lock").get(&id)
                {
                    let _ = tx.send(StreamMsg::Chunk(data));
                }
            }
            FromAgent::End { id, ok, error } => {
                let pending = connection.pending.lock().expect("pending lock").remove(&id);
                match pending {
                    Some(Pending::Stream(tx)) => {
                        let _ = tx.send(StreamMsg::End(if ok {
                            Ok(())
                        } else {
                            Err(error.unwrap_or_else(|| "failed".into()))
                        }));
                    }
                    Some(Pending::Res(tx)) => {
                        let _ = tx.send(if ok {
                            Ok(Value::Null)
                        } else {
                            Err(error.unwrap_or_else(|| "failed".into()))
                        });
                    }
                    None => {}
                }
                *connection.cache.lock().expect("cache lock") = None;
                metrics::invalidate(&agent_id);
                emit(&agent_id, "docker", Value::Null);
            }
            FromAgent::Event { kind, data } => {
                if kind == "stats" {
                    let mut host = Value::Null;
                    if let Ok(stats) = serde_json::from_value::<Stats>(data) {
                        patch_stats(&connection, &stats);
                        host = serde_json::json!({ "host": &stats.host });
                        metrics::ingest(&agent_id, stats);
                    }
                    emit(&agent_id, &kind, host);
                } else {
                    *connection.cache.lock().expect("cache lock") = None;
                    metrics::invalidate(&agent_id);
                    emit(&agent_id, &kind, data);
                }
            }
        }
    }

    let removed = {
        let mut conns = CONNS.write().expect("hub lock");
        if conns.get(&agent_id).is_some_and(|c| c.session == session) {
            conns.remove(&agent_id);
            true
        } else {
            false
        }
    };
    if removed {
        state::touch_agent(&agent_id).await;
        emit(&agent_id, "offline", Value::Null);
        tracing::info!("agent {agent_id} disconnected");
    }
    for (_, pending) in connection.pending.lock().expect("pending lock").drain() {
        match pending {
            Pending::Res(tx) => {
                let _ = tx.send(Err("agent disconnected".into()));
            }
            Pending::Stream(tx) => {
                let _ = tx.send(StreamMsg::End(Err("agent disconnected".into())));
            }
        }
    }
    writer.abort();
}
