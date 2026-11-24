use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

use crate::{
    components::ui::{button::job_button, icon::Icon},
    composables::use_host::Host,
    server::utils::{format::ago, state},
};

#[component]
pub async fn update_check(cx: &Cx, host: &Host) -> Result<impl View> {
    let id = host.record.id.clone();
    let checked = host.state.as_ref().and_then(|s| s.updates_checked_at);
    let updates = host.updates_available();
    let now = state::now();
    Ok(view! {
        <div class="inline">
            <span class="muted">
                if updates > 0 {
                    (updates) " updates · "
                }
                if let Some(t) = checked {
                    "checked " (ago(now - t))
                } else {
                    "never checked"
                }
            </span>
            if updates > 0 {
                job_button(url: format!("/api/hosts/{id}/updates/apply"), title: "Updating all", label: "Update all", glyph: Icon::Update, tone: "warn", confirm: Some(format!("Pull and recreate every service with an update available ({updates} images)?")))
            }
            job_button(url: format!("/api/hosts/{id}/updates/check"), title: "Checking for updates", label: "Check for updates", glyph: Icon::Refresh)
        </div>
    })
}

#[component]
pub async fn host_select(
    cx: &Cx,
    #[into] current: String,
    #[default] variant: &str,
) -> Result<impl View> {
    let agents = state::agents().await;
    Ok(view! {
        <select data-host-select="" data-select=(format!("ghost host-select {variant}")) aria-label="Host">
            for a in agents {
                <option value=(a.id.clone()) selected=(a.id == current)>(a.name)</option>
            }
        </select>
    })
}

#[component]
pub async fn host_head(cx: &Cx, host: &Host) -> Result<impl View> {
    let info = host.record.info.clone();
    Ok(view! {
        <div data-live="head">
        <header class="page-head">
            <div class="title">
                <h1>
                    <i class=(if host.online { "dot ok" } else { "dot off" })></i>
                    host_select(current: host.record.id.clone())
                </h1>
                <p class="sub">
                    if let Some(info) = info {
                        (info.hostname) " · " (info.os) "/" (info.arch)
                        if let Some(d) = info.docker {
                            " · docker " (d)
                        }
                    } else {
                        "never connected"
                    }
                </p>
            </div>
        </header>
        if !host.online {
            <div class="banner bad">"This agent is offline. It reconnects on its own when the host and the panel are reachable."</div>
        } else if let Some(e) = host.error.clone() {
            <div class="banner bad">(e)</div>
        } else if let Some(s) = host.state.as_ref() {
            if !s.stacks_dir_ok {
                <div class="banner warn">
                    "The stacks path " <code>(s.stacks_dir.clone())</code> " does not exist on this host. Create it or re-run " <code>"sudo senn-agent install --stacks-dir …"</code> " there."
                </div>
            }
        }
        </div>
    })
}
