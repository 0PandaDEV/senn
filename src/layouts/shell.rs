use topcoat::{
    Result,
    context::Cx,
    view::{Child, View, component, view},
};

use crate::{
    components::ui::{
        icon::{Icon, icon},
        nav::nav,
    },
    components::{avatar::avatar, host_head::host_select},
    server::utils::{auth::read_cookie, hub, state},
};

#[component]
pub async fn shell(
    cx: &Cx,
    #[default] section: &str,
    #[default] host: &str,
    #[default] tab: &str,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let agents = state::agents().await;
    let me = state::profile().await;
    let rows: Vec<(String, String, bool)> = agents
        .iter()
        .map(|a| (a.id.clone(), a.name.clone(), hub::online(&a.id)))
        .collect();
    let remembered = read_cookie(cx, "senn_host");
    let current = if !host.is_empty() {
        host.to_string()
    } else {
        remembered
            .filter(|h| agents.iter().any(|a| &a.id == h))
            .or_else(|| agents.first().map(|a| a.id.clone()))
            .unwrap_or_default()
    };
    let current_name = agents
        .iter()
        .find(|a| a.id == current)
        .map(|a| a.name.clone())
        .unwrap_or_default();
    let counts = if current.is_empty() || !hub::online(&current) {
        None
    } else {
        hub::host_state(&current)
            .await
            .ok()
            .map(|s| (s.stacks.len(), s.containers.len()))
    };
    let tabs: [(&str, &str, Icon, Option<usize>); 7] = [
        ("stacks", "Stacks", Icon::Layers, counts.map(|c| c.0)),
        ("containers", "Containers", Icon::Box, counts.map(|c| c.1)),
        ("images", "Images", Icon::Package, None),
        ("volumes", "Volumes", Icon::HardDrive, None),
        ("networks", "Networks", Icon::Network, None),
        ("logs", "Logs", Icon::Terminal, None),
        ("files", "Files", Icon::Folder, None),
    ];
    Ok(view! {
        <div class="shell">
            <header class="topbar">
                <button type="button" class="menu" data-menu="" aria-label="Menu">
                    icon(name: Icon::Menu)
                </button>
                <a class="brand" (nav(cx, "/"))>
                    <img class="brand-logo" src="/logo.svg" width="19" height="16" alt="">
                    <span>"Senn"</span>
                </a>
                if !current.is_empty() {
                    host_select(current: current.clone(), variant: "right")
                }
            </header>
            <div class="scrim" data-menu-close=""></div>
            <aside class="sidebar" data-live="side">
                <a class="brand" (nav(cx, "/"))>
                    <img class="brand-logo" src="/logo.svg" width="19" height="16" alt="">
                    <span>"Senn"</span>
                </a>
                <nav class="nav">
                    <a (nav(cx, "/")) class=(if section == "overview" { "active" } else { "" })>
                        icon(name: Icon::Home)
                        "Overview"
                    </a>
                    <a (nav(cx, "/agents")) class=(if section == "agents" { "active" } else { "" })>
                        icon(name: Icon::Server)
                        "Agents"
                    </a>
                </nav>
                <div class="label">"Hosts"</div>
                <nav class="nav hosts">
                    for (id, name, online) in rows {
                        <a (nav(cx, format!("/hosts/{id}"))) class=(if host == id && tab == "metrics" { "active" } else { "" })>
                            <i class=(if online { "dot ok" } else { "dot off" })></i>
                            <span>(name)</span>
                        </a>
                    }
                    if agents.is_empty() {
                        <p class="hint">"No agents yet"</p>
                    }
                </nav>
                if !current.is_empty() {
                    <div class="label">(current_name)</div>
                    <nav class="nav">
                        for (key, label, glyph, count) in tabs {
                            <a (nav(cx, format!("/hosts/{current}/{key}"))) class=(if host == current && tab == key { "active" } else { "" })>
                                icon(name: glyph)
                                <span>(label)</span>
                                if let Some(n) = count {
                                    <b class="num">(n)</b>
                                }
                            </a>
                        }
                    </nav>
                }
                <div class="me">
                    <a class="me-profile" (nav(cx, "/settings")) title="Account and settings">
                        avatar(profile: me.clone())
                        <span>(me.name())</span>
                    </a>
                    <a class=(if section == "settings" { "icon-button active" } else { "icon-button" }) (nav(cx, "/settings")) title="Settings" aria-label="Settings">
                        icon(name: Icon::Settings)
                    </a>
                    <button type="button" class="icon-button" data-api="/api/auth/logout" data-redirect="/login" title="Sign out" aria-label="Sign out">
                        icon(name: Icon::Logout)
                    </button>
                </div>
            </aside>
            <main class="main">(child)</main>
        </div>
    })
}
