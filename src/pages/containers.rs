use topcoat::{
    Result,
    context::Cx,
    router::{error::RouterErrorExt, page},
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::{
        badges::{ports, state_pill, usage},
        host_head::{host_head, update_check},
        metrics_panel::metrics_panel,
        ui::{
            button::{api_button, job_button},
            icon::{Icon, icon},
            nav::nav,
            policy::policy_select,
        },
    },
    composables::use_host::{Usage, load},
    layouts::shell::shell,
    server::utils::params,
};

#[page("/hosts/{agent}/containers", with(meta("Containers · Senn")), with(PageMeta { public: false }))]
pub async fn containers(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let host_name = host.record.link_host();
    let list = host
        .state
        .as_ref()
        .map(|s| s.containers.clone())
        .unwrap_or_default();
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "containers",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                <div data-live="containers">
                <div class="toolbar">
                    <h2>"Containers"</h2>
                    update_check(host: &host)
                </div>
                if list.is_empty() {
                    <section class="empty small"><p>"No containers."</p></section>
                } else {
                    <div class="table-wrap"><table class="table" data-minor="3 4">
                        <thead><tr><th>"Name"</th><th>"State"</th><th>"Usage"</th><th>"Ports"</th><th>"Updates"</th><th></th></tr></thead>
                        <tbody>
                            for c in list {
                                let update = host.has_update(&c);
                                <tr class="row">
                                    <td>
                                        <a class="strong go" (nav(cx, format!("/hosts/{id}/containers/{}", c.id)))>
                                            if update {
                                                icon(name: Icon::Update, size: 14)
                                            }
                                            (c.name.clone())
                                        </a>
                                        <div class="sub" title=(c.image.clone())>
                                            if let Some(s) = c.stack.clone() {
                                                <a (nav(cx, format!("/hosts/{id}/stacks/{s}")))>(s)</a>
                                                " · "
                                            }
                                            (c.image.clone())
                                        </div>
                                    </td>
                                    <td>state_pill(state: &c.state, health: c.health.as_deref())</td>
                                    <td>usage(u: Usage::of(&c))</td>
                                    <td>ports(ports: &c.ports, host: host_name.clone())</td>
                                    <td>
                                        if let (Some(st), Some(svc)) = (c.stack.clone(), c.service.clone()) {
                                            policy_select(agent: id.as_str(), scope: format!("{st}/{svc}"), current: host.record.config.policies.get(&format!("{st}/{svc}")).cloned().unwrap_or_default(), inherit: format!("Inherit ({})", c.update_policy))
                                        }
                                    </td>
                                    <td><div class="row-actions">
                                        if c.state == "running" {
                                            api_button(url: format!("/api/hosts/{id}/containers/{}/stop", c.id), label: "Stop", glyph: Icon::Stop)
                                        } else {
                                            api_button(url: format!("/api/hosts/{id}/containers/{}/start", c.id), label: "Start", glyph: Icon::Play)
                                        }
                                        api_button(url: format!("/api/hosts/{id}/containers/{}/restart", c.id), label: "Restart", glyph: Icon::Restart)
                                        if let (Some(st), Some(svc)) = (c.stack.clone(), c.service.clone()) {
                                            job_button(url: format!("/api/hosts/{id}/stacks/{st}/update?service={svc}"), title: format!("{st}/{svc} · update"), label: "Pull and recreate", glyph: Icon::Update, tone: if update { "warn" } else { "" })
                                        }
                                        api_button(url: format!("/api/hosts/{id}/containers/{}/remove", c.id), label: "Remove", glyph: Icon::Trash, tone: "danger", confirm: Some(format!("Remove container {}?", c.name)))
                                    </div></td>
                                </tr>
                            }
                        </tbody>
                    </table></div>
                }
                </div>
            </div>
        )
    })
}

#[page("/hosts/{agent}/containers/{container}", with(meta("Container · Senn")), with(PageMeta { public: false }))]
pub async fn container(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let cid = params::container(cx).to_string();
    let found = host.state.as_ref().and_then(|s| {
        s.containers
            .iter()
            .find(|c| c.id.starts_with(&cid) || c.name == cid)
            .cloned()
    });
    if host.online && host.state.is_some() && found.is_none() {
        None::<()>.ok_or_not_found()?;
    }
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "containers",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                <div class="crumbs">
                    <a (nav(cx, format!("/hosts/{id}/containers")))>"Containers"</a>
                    icon(name: Icon::Chevron, size: 14)
                    <strong>(found.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| cid.clone()))</strong>
                </div>
                if let Some(c) = found.clone() {
                    <div class="toolbar">
                        <div class="inline">
                            <h2>(c.name.clone())</h2>
                            state_pill(state: &c.state, health: c.health.as_deref())
                            <span class="muted">(c.image.clone())</span>
                        </div>
                        <div class="row-actions">
                            api_button(url: format!("/api/hosts/{id}/containers/{}/start", c.id), label: "Start", glyph: Icon::Play, text: true)
                            api_button(url: format!("/api/hosts/{id}/containers/{}/stop", c.id), label: "Stop", glyph: Icon::Stop, text: true)
                            api_button(url: format!("/api/hosts/{id}/containers/{}/restart", c.id), label: "Restart", glyph: Icon::Restart, text: true)
                        </div>
                    </div>
                    <div class="tabbar" data-tabs="">
                        <button type="button" class="active" data-tab="metrics">"Metrics"</button>
                        <button type="button" data-tab="logs">"Logs"</button>
                    </div>
                    <section data-pane="metrics">
                        metrics_panel(kind: "container", host: id.as_str(), name: c.name.as_str())
                    </section>
                    <section data-pane="logs" hidden=(true)>
                        <div data-logs=(format!("/api/hosts/{id}/logs?container={}", c.id))></div>
                    </section>
                }
            </div>
        )
    })
}
