use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

use crate::{
    proto::HostStats,
    server::utils::{
        format::{ago, bits, bytes, duration, rate},
        hub,
        state::{self, AgentInfo},
    },
};

pub struct Card {
    pub id: String,
    name: String,
    online: bool,
    stacks: usize,
    running: usize,
    total: usize,
    updates: usize,
    seen: Option<i64>,
    info: Option<AgentInfo>,
    host: Option<HostStats>,
    counts: Counts,
}

#[derive(Default)]
pub struct Counts {
    running: usize,
    stopped: usize,
    paused: usize,
    restarting: usize,
    unhealthy: usize,
    healthy: usize,
}

fn pct(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        used as f32 / total as f32 * 100.0
    }
}

fn tone(p: f32) -> &'static str {
    if p >= 90.0 {
        "bad"
    } else if p >= 70.0 {
        "warn"
    } else {
        "ok"
    }
}

pub struct Summary {
    pub hosts: usize,
    pub online: usize,
    pub running: usize,
    pub total: usize,
    pub stacks: usize,
    pub updates: usize,
}

pub fn summary(cards: &[Card]) -> Summary {
    Summary {
        hosts: cards.len(),
        online: cards.iter().filter(|c| c.online).count(),
        running: cards.iter().map(|c| c.running).sum(),
        total: cards.iter().map(|c| c.total).sum(),
        stacks: cards.iter().map(|c| c.stacks).sum(),
        updates: cards.iter().map(|c| c.updates).sum(),
    }
}

pub async fn cards() -> Vec<Card> {
    let mut out = Vec::new();
    for a in state::agents().await {
        let online = hub::online(&a.id);
        let st = if online {
            hub::host_state(&a.id).await.ok()
        } else {
            None
        };
        let host = st.as_ref().and_then(|s| s.host.clone());
        let mut counts = Counts::default();
        for c in st
            .as_ref()
            .map(|s| s.containers.as_slice())
            .unwrap_or_default()
        {
            match c.state.as_str() {
                "running" => counts.running += 1,
                "paused" => counts.paused += 1,
                "restarting" => counts.restarting += 1,
                _ => counts.stopped += 1,
            }
            match c.health.as_deref() {
                Some("unhealthy") => counts.unhealthy += 1,
                Some("healthy") => counts.healthy += 1,
                _ => {}
            }
        }
        let (stacks, running, total, updates) = match &st {
            Some(s) => (
                s.stacks.len(),
                s.containers.iter().filter(|c| c.state == "running").count(),
                s.containers.len(),
                s.updates.iter().filter(|u| u.status == "available").count(),
            ),
            None => (0, 0, 0, 0),
        };
        out.push(Card {
            id: a.id,
            name: a.name,
            online,
            stacks,
            running,
            total,
            updates,
            seen: a.last_seen,
            info: a.info,
            host,
            counts,
        });
    }
    out
}

#[component]
pub async fn node_card(cx: &Cx, c: Card, #[default] bare: bool) -> Result<impl View> {
    let now = state::now();
    Ok(view! {
        if !bare {
            <div class="host-top">
                <i class=(if c.online { "dot ok" } else { "dot off" })></i>
                <strong>(c.name.clone())</strong>
                if c.updates > 0 {
                    <span class="tag warn">(c.updates) " updates"</span>
                }
                if let Some(h) = c.host.as_ref() {
                    <span class="host-uptime">"up " (duration(h.uptime))</span>
                }
            </div>
        }
        if c.online {
            if let Some(h) = c.host.clone() {
                let mem = pct(h.mem_used, h.mem_total);
                let disk = pct(h.disk_used, h.disk_total);
                <div class="meters">
                    <div class="meter-tile">
                        <div class="meter-head"><span>"CPU"</span><b>(format!("{:.1}%", h.cpu))</b></div>
                        <span class="meter"><i class=(tone(h.cpu)) style=(format!("width:{:.1}%", h.cpu.min(100.0)))></i></span>
                        <span class="meter-sub">(format!("{} cores · load {:.2}", h.cores, h.load[0]))</span>
                    </div>
                    <div class="meter-tile">
                        <div class="meter-head"><span>"Memory"</span><b>(format!("{mem:.0}%"))</b></div>
                        <span class="meter"><i class=(tone(mem)) style=(format!("width:{mem:.1}%"))></i></span>
                        <span class="meter-sub">(bytes(h.mem_used)) " / " (bytes(h.mem_total))</span>
                    </div>
                    <div class="meter-tile">
                        <div class="meter-head"><span>"Disk"</span><b>(format!("{disk:.0}%"))</b></div>
                        <span class="meter"><i class=(tone(disk)) style=(format!("width:{disk:.1}%"))></i></span>
                        <span class="meter-sub">(bytes(h.disk_used)) " / " (bytes(h.disk_total))</span>
                    </div>
                </div>
            }
            <div class="host-containers">
                <span class="host-label">"Containers"</span>
                <b>(c.running) <small>" / " (c.total)</small></b>
                <div class="chips">
                    if c.counts.stopped > 0 {
                        <span class="pill bad"><i></i>(c.counts.stopped) " stopped"</span>
                    }
                    if c.counts.paused > 0 {
                        <span class="pill warn"><i></i>(c.counts.paused) " paused"</span>
                    }
                    if c.counts.restarting > 0 {
                        <span class="pill warn"><i></i>(c.counts.restarting) " restarting"</span>
                    }
                    if c.counts.unhealthy > 0 {
                        <span class="pill bad"><i></i>(c.counts.unhealthy) " unhealthy"</span>
                    } else if c.counts.running == c.total && c.total > 0 {
                        <span class="pill ok"><i></i>"all healthy"</span>
                    }
                </div>
            </div>
            if let Some(h) = c.host.clone() {
                <dl class="io">
                    if bare {
                        <div><dt>"Uptime"</dt><dd>(duration(h.uptime))</dd></div>
                        <div><dt>"Load"</dt><dd>(format!("{:.2}", h.load[0])) <small>(format!("{:.2} · {:.2}", h.load[1], h.load[2]))</small></dd></div>
                        <div><dt>"Swap"</dt><dd>(bytes(h.swap_used)) <small>(format!("of {}", bytes(h.swap_total)))</small></dd></div>
                        if let Some(x) = h.extra.clone() {
                            <div><dt>"Processes"</dt><dd>(x.procs_total) <small>(format!("{} running", x.procs_running))</small></dd></div>
                            <div><dt>"TCP sockets"</dt><dd>(x.tcp_inuse)</dd></div>
                            if let Some(t) = x.temp {
                                <div><dt>"Temperature"</dt><dd>(format!("{t:.0} °C"))</dd></div>
                            }
                            if let Some(p) = x.pressure_cpu {
                                <div><dt>"CPU pressure"</dt><dd>(format!("{p:.1}%"))</dd></div>
                            }
                            <div><dt>"Disk busy"</dt><dd>(format!("{:.0}%", x.io_busy))</dd></div>
                        }
                    }
                    if let Some(d) = h.docker.clone() {
                        <div><dt>"Images"</dt><dd>(d.images) <small>(bytes(d.images_size))</small></dd></div>
                        <div><dt>"Volumes"</dt><dd>(d.volumes)</dd></div>
                        <div><dt>"Networks"</dt><dd>(d.networks)</dd></div>
                    }
                    <div><dt>"Stacks"</dt><dd>(c.stacks)</dd></div>
                    <div><dt>"Net in"</dt><dd>(bits(h.net_rx))</dd></div>
                    <div><dt>"Net out"</dt><dd>(bits(h.net_tx))</dd></div>
                    <div><dt>"Disk read"</dt><dd>(rate(h.io_read))</dd></div>
                    <div><dt>"Disk write"</dt><dd>(rate(h.io_write))</dd></div>
                </dl>
            } else {
                <p class="muted small">"Resource stats need the latest agent."</p>
            }
            if let Some(info) = c.info.clone() {
                <p class="host-sub">
                    (info.hostname) " · " (info.os) "/" (info.arch)
                    if let Some(d) = info.docker {
                        " · docker " (d)
                    }
                    if let Some(h) = c.host.as_ref() {
                        " · kernel " (h.kernel.clone())
                    }
                    " · agent " (info.version)
                </p>
            }
        } else {
            <p class="muted">
                if let Some(t) = c.seen {
                    "offline · last seen " (ago(now - t))
                } else {
                    "waiting for first connection"
                }
            </p>
        }
    })
}
