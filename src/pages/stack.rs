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
        host_head::host_head,
        metrics_panel::metrics_panel,
        ui::{
            button::{api_button, job_button},
            icon::{Icon, icon},
            nav::nav,
        },
    },
    composables::use_host::{load, stack_issues, stack_summary, Usage},
    layouts::shell::shell,
    server::utils::params,
};

#[page("/hosts/{agent}/stacks/{stack}", with(meta("Stack · Senn")), with(PageMeta { public: false }))]
pub async fn stack(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let name = params::stack(cx).to_string();
    if host.online && host.state.is_some() && host.stack(&name).is_none() {
        None::<()>.ok_or_not_found()?;
    }
    let id = host.record.id.clone();
    let host_name = host.record.link_host();
    let info = host.stack(&name).cloned();
    let containers = host
        .stack_containers(&name)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let (running, total) = stack_summary(&containers.iter().collect::<Vec<_>>());
    let base = format!("/api/hosts/{id}/stacks/{name}");
    let updates = host.stack_updates(&name);
    let all = containers.iter().collect::<Vec<_>>();
    let used = Usage::sum(&all);
    let issues = stack_issues(&all);
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "stacks",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                <div class="crumbs">
                    <a (nav(cx, format!("/hosts/{id}/stacks")))>"Stacks"</a>
                    icon(name: Icon::Chevron, size: 14)
                    <strong>(name.clone())</strong>
                </div>
                <div data-live="toolbar">
                <div class="toolbar">
                    <div class="inline">
                        <h2>(name.clone())</h2>
                        if total > 0 {
                            <span class=(format!("pill {}", if running == total { "ok" } else if running == 0 { "bad" } else { "warn" }))><i></i>(running) "/" (total) " running"</span>
                        } else {
                            <span class="pill idle"><i></i>"not deployed"</span>
                        }
                        if issues > 0 {
                            <span class="pill bad"><i></i>(issues) " unhealthy"</span>
                        }
                        usage(u: used)
                    </div>
                    <div class="row-actions">
                        job_button(url: format!("{base}/up"), title: format!("{name} · up"), label: "Up", glyph: Icon::Play, text: true)
                        job_button(url: format!("{base}/update"), title: format!("{name} · update"), label: "Update", glyph: Icon::Update, text: true, tone: if updates > 0 { "warn" } else { "" })
                        job_button(url: format!("{base}/restart"), title: format!("{name} · restart"), label: "Restart", glyph: Icon::Restart)
                        job_button(url: format!("{base}/stop"), title: format!("{name} · stop"), label: "Stop", glyph: Icon::Stop)
                        job_button(url: format!("{base}/pull"), title: format!("{name} · pull"), label: "Pull images", glyph: Icon::Pull)
                        job_button(url: format!("{base}/down"), title: format!("{name} · down"), label: "Down", glyph: Icon::Power, tone: "danger", confirm: Some(format!("Stop and remove the containers of {name}? Volumes are kept.")))
                        job_button(url: format!("{base}/delete"), title: format!("{name} · delete"), label: "Delete", glyph: Icon::Trash, tone: "danger", confirm: Some(format!("Delete stack {name}? This runs compose down and removes the folder {} with everything in it. Named volumes are kept.", host.stack_dir(&name))), redirect: Some(format!("/hosts/{id}/stacks")))
                    </div>
                </div>
                if let Some(i) = info.clone() {
                    if let Some(e) = i.error {
                        <div class="banner bad"><pre>(e)</pre></div>
                    }
                }

                </div>

                <div class="tabbar" data-tabs="">
                    <button type="button" class="active" data-tab="services">"Services"</button>
                    <button type="button" data-tab="edit">"Edit"</button>
                    <button type="button" data-tab="metrics">"Metrics"</button>
                    <button type="button" data-tab="logs">"Logs"</button>
                </div>

                <section data-pane="services">
                <div data-live="services">
                    if containers.is_empty() {
                        <section class="empty small"><p>"Nothing is deployed. Press Up to create the containers."</p></section>
                    } else {
                        <div class="table-wrap"><table class="table" data-minor="3 4">
                            <thead><tr><th>"Service"</th><th>"State"</th><th>"Usage"</th><th>"Ports"</th><th></th></tr></thead>
                            <tbody>
                                for c in containers {
                                    let update = host.has_update(&c);
                                    let svc = c.service.clone().unwrap_or_else(|| c.name.clone());
                                    <tr class="row">
                                        <td>
                                            <a class="strong go" (nav(cx, format!("/hosts/{id}/containers/{}", c.id)))>
                                                if update {
                                                    icon(name: Icon::Update, size: 14)
                                                }
                                                (svc.clone())
                                            </a>
                                            <div class="sub" title=(format!("{} · {} · update policy {}", c.name, c.image, c.update_policy))>(c.image.clone())</div>
                                        </td>
                                        <td>state_pill(state: &c.state, health: c.health.as_deref())</td>
                                        <td>usage(u: Usage::of(&c))</td>
                                        <td>ports(ports: &c.ports, host: host_name.clone())</td>
                                        <td><div class="row-actions">
                                            if c.state == "running" {
                                                api_button(url: format!("/api/hosts/{id}/containers/{}/stop", c.id), label: "Stop", glyph: Icon::Stop)
                                            } else {
                                                api_button(url: format!("/api/hosts/{id}/containers/{}/start", c.id), label: "Start", glyph: Icon::Play)
                                            }
                                            api_button(url: format!("/api/hosts/{id}/containers/{}/restart", c.id), label: "Restart", glyph: Icon::Restart)
                                            job_button(url: format!("{base}/update?service={svc}"), title: format!("{name}/{svc} · update"), label: "Pull and recreate", glyph: Icon::Update, tone: if update { "warn" } else { "" })
                                        </div></td>
                                    </tr>
                                }
                            </tbody>
                        </table></div>
                    }
                </div>
                </section>
                <section data-pane="edit" hidden=(true)>
                    <div class="edit-stack">
                        <div class="edit-block">
                            <h3>"Environment"</h3>
                            <div data-env="" data-agent=(id.clone()) data-stack=(name.clone()) data-dir=(host.stack_dir(&name)) data-compose=(info.as_ref().map(|i| i.file.clone()).unwrap_or_default())></div>
                        </div>
                        <div class="edit-block">
                            <h3>"Files"</h3>
                            <div data-files="" data-agent=(id.clone()) data-root=(name.clone()) data-abs=(host.stacks_dir())></div>
                        </div>
                    </div>
                </section>
                <section data-pane="metrics" hidden=(true)>
                    metrics_panel(kind: "stack", host: id.as_str(), name: name.as_str())
                </section>
                <section data-pane="logs" hidden=(true)>
                    <div data-logs=(format!("/api/hosts/{id}/logs?stack={name}")) data-multi=""></div>
                </section>
            </div>
        )
    })
}
