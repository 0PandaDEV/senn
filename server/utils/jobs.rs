use std::{
    collections::VecDeque,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use serde_json::{Value, json};
use tokio::sync::broadcast;

use super::{
    hub::{StreamHandle, StreamMsg},
    state,
};

const KEEP_JOBS: usize = 20;
const KEEP_LINES: usize = 3000;

#[derive(Clone)]
struct Job {
    id: u64,
    title: String,
    key: String,
    state: &'static str,
    error: Option<String>,
    started: i64,
    lines: Vec<String>,
}

static JOBS: LazyLock<Mutex<VecDeque<Job>>> = LazyLock::new(Default::default);
static NEXT: AtomicU64 = AtomicU64::new(1);
static EVENTS: LazyLock<broadcast::Sender<Value>> = LazyLock::new(|| broadcast::channel(512).0);

pub fn subscribe() -> broadcast::Receiver<Value> {
    EVENTS.subscribe()
}

fn emit(value: Value) {
    let _ = EVENTS.send(value);
}

fn update(id: u64, f: impl FnOnce(&mut Job)) {
    if let Some(job) = JOBS
        .lock()
        .expect("jobs lock")
        .iter_mut()
        .find(|j| j.id == id)
    {
        f(job);
    }
}

pub fn start(title: String, key: String, mut handle: StreamHandle) -> u64 {
    let id = ((state::now() as u64) << 16) | (NEXT.fetch_add(1, Ordering::Relaxed) & 0xffff);
    let job = Job {
        id,
        title: title.clone(),
        key: key.clone(),
        state: "running",
        error: None,
        started: state::now(),
        lines: Vec::new(),
    };
    {
        let mut jobs = JOBS.lock().expect("jobs lock");
        jobs.push_back(job);
        while jobs.len() > KEEP_JOBS {
            jobs.pop_front();
        }
    }
    emit(json!({ "t": "job", "ev": "start", "id": id, "title": title, "key": key }));
    tokio::spawn(async move {
        let mut failure = Some("connection to the agent was lost".to_string());
        let mut ok = false;
        while let Some(msg) = handle.rx.recv().await {
            match msg {
                StreamMsg::Chunk(line) => {
                    update(id, |j| {
                        j.lines.push(line.clone());
                        if j.lines.len() > KEEP_LINES {
                            j.lines.drain(..j.lines.len() - KEEP_LINES);
                        }
                    });
                    emit(json!({ "t": "job", "ev": "line", "id": id, "line": line }));
                }
                StreamMsg::End(result) => {
                    ok = result.is_ok();
                    failure = result.err();
                    break;
                }
            }
        }
        let error = if ok { None } else { failure };
        update(id, |j| {
            j.state = if ok { "done" } else { "failed" };
            j.error = error.clone();
        });
        emit(
            json!({ "t": "job", "ev": "end", "id": id, "ok": ok, "error": error, "title": title }),
        );
    });
    id
}

pub fn snapshot() -> Vec<Value> {
    JOBS.lock()
        .expect("jobs lock")
        .iter()
        .map(|j| {
            json!({
                "id": j.id,
                "title": j.title,
                "key": j.key,
                "state": j.state,
                "error": j.error,
                "started": j.started,
                "lines": j.lines,
            })
        })
        .collect()
}
