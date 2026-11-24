use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

use crate::{
    components::ui::icon::{Icon, icon},
    composables::use_host::Usage,
    proto::PortInfo,
    server::utils::format::{bits, bytes, rate, short_bits, short_bytes},
};

#[component]
pub async fn state_pill(
    cx: &Cx,
    state: &str,
    #[default] health: Option<&str>,
) -> Result<impl View> {
    let tone = match (state, health) {
        (_, Some("unhealthy")) => "bad",
        ("running", Some("starting")) => "warn",
        ("running", _) => "ok",
        ("restarting", _) | ("paused", _) => "warn",
        ("exited", _) | ("dead", _) => "bad",
        _ => "idle",
    };
    let label = match health {
        Some(h) if state == "running" && h != "healthy" => format!("{state} · {h}"),
        _ => state.to_string(),
    };
    Ok(view! {
        <span class=(format!("pill {tone}"))><i></i>(label)</span>
    })
}

#[component]
pub async fn update_badge(cx: &Cx, status: &str) -> Result<impl View> {
    let (tone, text) = match status {
        "available" => ("warn", "update available"),
        "current" => ("ok", "up to date"),
        "pinned" => ("idle", "pinned"),
        "local" => ("idle", "local image"),
        "error" => ("bad", "check failed"),
        _ => ("idle", "not checked"),
    };
    Ok(view! { <span class=(format!("tag {tone}"))>(text)</span> })
}

#[component]
pub async fn usage(cx: &Cx, u: Usage) -> Result<impl View> {
    let mem = u.mem.filter(|m| *m > 0);
    let mut detail = Vec::new();
    if let Some((rx, tx)) = u.net {
        detail.push(format!("Network in {} · out {}", bits(rx), bits(tx)));
    }
    if let Some((r, w)) = u.io {
        detail.push(format!("Disk read {} · write {}", rate(r), rate(w)));
    }
    let dash = || "—".to_string();
    Ok(view! {
        if u.cpu.is_some() || mem.is_some() {
            <div class="usage" title=(detail.join("\n"))>
                <span><small>"CPU"</small><b>(u.cpu.map(|c| format!("{c:.1}%")).unwrap_or_else(dash))</b></span>
                <span><small>"MEM"</small><b>(mem.map(bytes).unwrap_or_else(dash))</b></span>
                <span><small>"NET"</small><b>(u.net.map(|(rx, tx)| format!("↓{} ↑{}", short_bits(rx), short_bits(tx))).unwrap_or_else(dash))</b></span>
                <span><small>"IO"</small><b>(u.io.map(|(r, w)| format!("R{} W{}", short_bytes(r), short_bytes(w))).unwrap_or_else(dash))</b></span>
            </div>
        } else {
            <span class="muted">"—"</span>
        }
    })
}

#[component]
pub async fn svc_chip(
    cx: &Cx,
    #[into] name: String,
    state: &str,
    update: bool,
) -> Result<impl View> {
    let tone = if update {
        "warn"
    } else if state == "running" {
        "ok"
    } else if state == "none" {
        "idle"
    } else {
        "bad"
    };
    Ok(view! {
        <span class=(format!("chip {tone}")) title=(if update { "update available" } else { "" })>
            if update {
                icon(name: Icon::Update, size: 12)
            }
            (name)
        </span>
    })
}

#[component]
pub async fn ports(cx: &Cx, ports: &[PortInfo], #[into] host: String) -> Result<impl View> {
    let mut list: Vec<(u16, u16, String, String)> = ports
        .iter()
        .filter_map(|p| {
            let public = p.public?;
            let ip =
                p.ip.as_deref()
                    .filter(|ip| !matches!(*ip, "0.0.0.0" | "::" | ""))
                    .unwrap_or(host.as_str());
            let ip = if ip.contains(':') {
                format!("[{ip}]")
            } else {
                ip.to_string()
            };
            Some((public, p.private, p.proto.clone(), ip))
        })
        .collect();
    list.sort_by(|a, b| (&a.2, a.0, a.1).cmp(&(&b.2, b.0, b.1)));
    list.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1 && a.2 == b.2);
    let mut groups: Vec<(u16, u16, u16, String, String)> = Vec::new();
    for (public, private, proto, ip) in list {
        if let Some(last) = groups.last_mut()
            && last.3 == proto
            && last.4 == ip
            && public == last.0 + (last.2 - last.1) + 1
            && private == last.2 + 1
        {
            last.2 = private;
            continue;
        }
        groups.push((public, private, private, proto, ip));
    }
    let label = |(public, from, to, proto, _): &(u16, u16, u16, String, String)| {
        let span = to - from;
        let ports = if span == 0 {
            format!("{public}:{from}")
        } else if *public == *from {
            format!("{from}-{to}")
        } else {
            format!("{public}-{}:{from}-{to}", public + span)
        };
        if proto == "tcp" {
            ports
        } else {
            format!("{ports}/{proto}")
        }
    };
    const SHOWN: usize = 2;
    let hidden: Vec<String> = groups.iter().skip(SHOWN).map(label).collect();
    let shown: Vec<(String, Option<String>)> = groups
        .iter()
        .take(SHOWN)
        .map(|g| {
            let link = (g.3 == "tcp" && g.1 == g.2).then(|| format!("http://{}:{}", g.4, g.0));
            (label(g), link)
        })
        .collect();
    Ok(view! {
        <div class="ports">
            for (text, link) in shown {
                if let Some(href) = link {
                    <a class="pill port" href=(href.clone()) target="_blank" rel="noopener" title=(format!("Open {href}"))>(text)</a>
                } else {
                    <span class="pill port">(text)</span>
                }
            }
            if !hidden.is_empty() {
                <span class="pill port more" title=(hidden.join("\n"))>(format!("+{}", hidden.len()))</span>
            }
        </div>
    })
}
