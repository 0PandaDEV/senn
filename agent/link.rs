use std::{sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::HeaderValue},
};

use crate::{
    config::Local,
    docker, handlers,
    proto::{FromAgent, ToAgent},
};
use handlers::Ctx;

fn ws_url(base: &str) -> String {
    let base = base.trim_end_matches('/');
    let swapped = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base.to_string()
    };
    format!("{swapped}/api/agent/connect")
}

pub async fn run(local: Local, ctx: Arc<Ctx>) {
    let mut delay = 1u64;
    loop {
        let started = std::time::Instant::now();
        match session(&local, ctx.clone()).await {
            Ok(()) => tracing::info!("disconnected from panel"),
            Err(e) => tracing::warn!("panel connection failed: {e}"),
        }
        if started.elapsed() > Duration::from_secs(30) {
            delay = 1;
        }
        tokio::time::sleep(Duration::from_secs(delay)).await;
        delay = (delay * 2).min(30);
    }
}

async fn session(local: &Local, ctx: Arc<Ctx>) -> Result<(), String> {
    let mut request = ws_url(&local.url)
        .into_client_request()
        .map_err(|e| e.to_string())?;
    request.headers_mut().insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", local.token)).map_err(|e| e.to_string())?,
    );
    let (socket, _) = connect_async(request).await.map_err(|e| e.to_string())?;
    tracing::info!("connected to {}", local.url);
    let (mut sink, mut stream) = socket.split();
    let (out, mut outbox) = mpsc::unbounded_channel::<FromAgent>();

    let docker_version = ctx.docker.version().await.ok().and_then(|v| v.version);
    out.send(FromAgent::Hello {
        version: env!("CARGO_PKG_VERSION").into(),
        hostname: hostname(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        docker: docker_version,
        address: primary_ip(),
    })
    .ok();

    let writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval(Duration::from_secs(20));
        loop {
            tokio::select! {
                msg = outbox.recv() => {
                    let Some(msg) = msg else { break };
                    let Ok(text) = serde_json::to_string(&msg) else { continue };
                    if sink.send(Message::text(text)).await.is_err() {
                        break;
                    }
                }
                _ = ping.tick() => {
                    if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let events = tokio::spawn(forward_events(ctx.clone(), out.clone()));
    let scheduler = tokio::spawn(schedule(ctx.clone(), out.clone()));
    let sampler = tokio::spawn(sample(ctx.clone(), out.clone()));
    let mut running: std::collections::HashMap<u64, (tokio::task::JoinHandle<()>, bool)> =
        std::collections::HashMap::new();

    let result = loop {
        let next = tokio::time::timeout(Duration::from_secs(60), stream.next()).await;
        let frame = match next {
            Err(_) => break Err("panel went silent".to_string()),
            Ok(None) => break Ok(()),
            Ok(Some(Err(e))) => break Err(e.to_string()),
            Ok(Some(Ok(f))) => f,
        };
        let Message::Text(text) = frame else {
            if matches!(frame, Message::Close(_)) {
                break Ok(());
            }
            continue;
        };
        let Ok(msg) = serde_json::from_str::<ToAgent>(&text) else {
            tracing::warn!("unreadable message from panel");
            let id = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v.get("id")?.as_u64());
            if let Some(id) = id {
                let _ = out.send(FromAgent::Res {
                    id,
                    ok: false,
                    data: serde_json::Value::Null,
                    error: Some("the agent does not support this request, update it".into()),
                });
            }
            continue;
        };
        running.retain(|_, (h, _)| !h.is_finished());
        match msg {
            ToAgent::Welcome { config } | ToAgent::Config { config } => {
                ctx.set_config(config);
            }
            ToAgent::Cancel { id } => {
                if let Some((h, _)) = running.remove(&id) {
                    h.abort();
                }
            }
            ToAgent::Req { id, op } => {
                if op.streams() {
                    let detach = !matches!(op, crate::proto::Op::Logs { .. });
                    running.insert(
                        id,
                        (
                            handlers::spawn_stream(ctx.clone(), id, op, out.clone()),
                            detach,
                        ),
                    );
                } else {
                    let ctx = ctx.clone();
                    let out = out.clone();
                    tokio::spawn(async move {
                        let res = handlers::handle(&ctx, op).await;
                        let _ = out.send(match res {
                            Ok(data) => FromAgent::Res {
                                id,
                                ok: true,
                                data,
                                error: None,
                            },
                            Err(e) => FromAgent::Res {
                                id,
                                ok: false,
                                data: serde_json::Value::Null,
                                error: Some(e),
                            },
                        });
                    });
                }
            }
        }
    };
    for (_, (h, detach)) in running {
        if !detach {
            h.abort();
        }
    }
    events.abort();
    scheduler.abort();
    sampler.abort();
    writer.abort();
    result
}

fn primary_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "unknown".into())
}

async fn forward_events(ctx: Arc<Ctx>, out: mpsc::UnboundedSender<FromAgent>) {
    let mut last = std::time::Instant::now() - Duration::from_secs(10);
    loop {
        let mut events = std::pin::pin!(docker::events(&ctx.docker));
        while let Some(e) = events.next().await {
            let notable = docker::notable(&e);
            if !notable && last.elapsed() < Duration::from_millis(300) {
                continue;
            }
            last = std::time::Instant::now();
            let get = |k: &str| e.attrs.get(k).cloned();
            let _ = out.send(FromAgent::Event {
                kind: "docker".into(),
                data: serde_json::json!({
                    "action": e.action,
                    "id": e.id,
                    "notable": notable,
                    "name": get("name"),
                    "image": get("image"),
                    "exit_code": get("exitCode"),
                    "stack": get("com.docker.compose.project"),
                    "service": get("com.docker.compose.service"),
                }),
            });
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn sample(ctx: Arc<Ctx>, out: mpsc::UnboundedSender<FromAgent>) {
    let mut prev = docker::Prev::new();
    let mut host_prev = crate::host::Prev::default();
    let mut last: Option<String> = None;
    let _ = docker::sample(&ctx.docker, &mut prev).await;
    let _ = crate::host::sample(&mut host_prev, &ctx.stacks_dir).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let mut tick: u64 = 0;
    let mut engine = None;
    loop {
        let (samples, errors) = docker::sample(&ctx.docker, &mut prev).await;
        let mut host = crate::host::sample(&mut host_prev, &ctx.stacks_dir).await;
        if tick % 30 == 0 {
            engine = docker::docker_stats(&ctx.docker).await;
        }
        tick += 1;
        host.docker = engine.clone();
        *ctx.host.write().expect("host lock") = Some(host.clone());
        let first = errors.first().cloned();
        if first != last {
            if let Some(e) = &first {
                tracing::warn!("container stats failed: {e}");
            }
            last = first;
        }
        let data = serde_json::json!({ "containers": &samples, "host": &host });
        *ctx.samples.write().expect("samples lock") = samples;
        let _ = out.send(FromAgent::Event {
            kind: "stats".into(),
            data,
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn schedule(ctx: Arc<Ctx>, out: mpsc::UnboundedSender<FromAgent>) {
    tokio::time::sleep(Duration::from_secs(20)).await;
    loop {
        let cfg = ctx.config();
        let interval = cfg.update_interval_secs.max(300);
        let due = ctx
            .updates
            .snapshot()
            .1
            .map(|t| crate::updates::now() - t >= interval as i64)
            .unwrap_or(true);
        if due {
            if let Err(e) = handlers::check_updates(&ctx, |_| {}).await {
                tracing::warn!("update check failed: {e}");
            } else {
                handlers::auto_update(&ctx, &out).await;
                let _ = out.send(FromAgent::Event {
                    kind: "updates".into(),
                    data: serde_json::Value::Null,
                });
            }
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
