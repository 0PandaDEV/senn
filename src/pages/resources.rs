use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::{
        host_head::host_head,
        ui::{
            button::{api_button, job_button},
            icon::Icon,
        },
        used_by::used_by,
    },
    composables::use_host::{Host, load},
    layouts::shell::shell,
    proto::{ImageRow, NetworkRow, Op, ResourceKind, VolumeRow},
    server::utils::{
        format::{ago, bytes, short_id},
        hub, state,
    },
};

async fn list<T: serde::de::DeserializeOwned>(
    host: &Host,
    kind: ResourceKind,
) -> std::result::Result<Vec<T>, String> {
    if !host.online {
        return Err("This agent is offline.".into());
    }
    let value = hub::request(&host.record.id, Op::Resources { kind }).await?;
    serde_json::from_value(value)
        .map_err(|_| "Update the agent on this host to manage this.".into())
}

fn body(id: &str) -> String {
    serde_json::json!({ "id": id }).to_string()
}

#[page("/hosts/{agent}/images", with(meta("Images · Senn")), with(PageMeta { public: false }))]
pub async fn images(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let rows = list::<ImageRow>(&host, ResourceKind::Image).await;
    let now = state::now();
    let summary = rows.as_ref().ok().map(|r| {
        let total: u64 = r.iter().map(|i| i.size).sum();
        let unused = r.iter().filter(|i| i.used_by.is_empty()).count();
        format!("{} images · {} · {unused} unused", r.len(), bytes(total))
    });
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "images",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                <div data-live="images">
                    <div class="toolbar">
                        <div class="inline">
                            <h2>"Images"</h2>
                            if let Some(t) = summary {
                                <span class="muted">(t)</span>
                            }
                        </div>
                        job_button(url: format!("/api/hosts/{id}/resources/images/prune"), title: "Prune images", label: "Prune", glyph: Icon::Trash, text: true, tone: "danger", confirm: Some("Remove every image no container uses? They will be pulled again when needed.".to_string()))
                    </div>
                    match rows {
                        Err(e) => {
                            <section class="empty small"><p>(e)</p></section>
                        }
                        Ok(rows) => {
                            <div class="table-wrap"><table class="table">
                                <thead><tr><th>"Image"</th><th>"Size"</th><th>"Used by"</th><th></th></tr></thead>
                                <tbody>
                                    for i in rows {
                                        let name = i.tags.first().cloned().unwrap_or_else(|| "<untagged>".into());
                                        <tr data-key=(i.id.clone())>
                                            <td>
                                                <span class="strong" title=(i.tags.join("\n"))>(name)</span>
                                                <div class="sub">
                                                    (short_id(&i.id).to_string()) " · " (ago(now - i.created))
                                                    if i.tags.len() > 1 {
                                                        " · " (format!("+{} tags", i.tags.len() - 1))
                                                    }
                                                </div>
                                            </td>
                                            <td class="num">(bytes(i.size))</td>
                                            <td>used_by(names: i.used_by.clone())</td>
                                            <td><div class="row-actions">
                                                api_button(url: format!("/api/hosts/{id}/resources/images/remove"), label: "Remove image", glyph: Icon::Trash, tone: "danger", confirm: Some(format!("Remove image {}?", i.tags.first().cloned().unwrap_or_else(|| short_id(&i.id).to_string()))), body: Some(body(&i.id)))
                                            </div></td>
                                        </tr>
                                    }
                                </tbody>
                            </table></div>
                        }
                    }
                </div>
            </div>
        )
    })
}

#[page("/hosts/{agent}/volumes", with(meta("Volumes · Senn")), with(PageMeta { public: false }))]
pub async fn volumes(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let rows = list::<VolumeRow>(&host, ResourceKind::Volume).await;
    let summary = rows.as_ref().ok().map(|r| {
        format!(
            "{} volumes · {} unused",
            r.len(),
            r.iter().filter(|v| v.used_by.is_empty()).count()
        )
    });
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "volumes",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                <div data-live="volumes">
                    <div class="toolbar">
                        <div class="inline">
                            <h2>"Volumes"</h2>
                            if let Some(t) = summary {
                                <span class="muted">(t)</span>
                            }
                        </div>
                        job_button(url: format!("/api/hosts/{id}/resources/volumes/prune"), title: "Prune volumes", label: "Prune", glyph: Icon::Trash, text: true, tone: "danger", confirm: Some("Delete every volume no container uses, named ones included? The data in them is gone for good.".to_string()))
                    </div>
                    match rows {
                        Err(e) => {
                            <section class="empty small"><p>(e)</p></section>
                        }
                        Ok(rows) => {
                            <div class="table-wrap"><table class="table">
                                <thead><tr><th>"Volume"</th><th>"Used by"</th><th></th></tr></thead>
                                <tbody>
                                    for v in rows {
                                        <tr data-key=(v.name.clone())>
                                            <td>
                                                <span class="strong" title=(v.name.clone())>(if v.anonymous { format!("{}…", &v.name[..12.min(v.name.len())]) } else { v.name.clone() })</span>
                                                <div class="sub" title=(v.mountpoint.clone())>
                                                    if let Some(s) = v.stack.clone() {
                                                        <a href=(format!("/hosts/{id}/stacks/{s}"))>(s)</a>
                                                        " · "
                                                    }
                                                    (v.driver.clone())
                                                    if v.anonymous {
                                                        " · anonymous"
                                                    }
                                                </div>
                                            </td>
                                            <td>used_by(names: v.used_by.clone())</td>
                                            <td><div class="row-actions">
                                                api_button(url: format!("/api/hosts/{id}/resources/volumes/remove"), label: "Delete volume", glyph: Icon::Trash, tone: "danger", confirm: Some(format!("Delete volume {}? The data in it is gone for good.", v.name)), body: Some(body(&v.name)))
                                            </div></td>
                                        </tr>
                                    }
                                </tbody>
                            </table></div>
                        }
                    }
                </div>
            </div>
        )
    })
}

#[page("/hosts/{agent}/networks", with(meta("Networks · Senn")), with(PageMeta { public: false }))]
pub async fn networks(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let rows = list::<NetworkRow>(&host, ResourceKind::Network).await;
    let summary = rows.as_ref().ok().map(|r| format!("{} networks", r.len()));
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "networks",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                <div data-live="networks">
                    <div class="toolbar">
                        <div class="inline">
                            <h2>"Networks"</h2>
                            if let Some(t) = summary {
                                <span class="muted">(t)</span>
                            }
                        </div>
                        job_button(url: format!("/api/hosts/{id}/resources/networks/prune"), title: "Prune networks", label: "Prune", glyph: Icon::Trash, text: true, tone: "danger", confirm: Some("Remove every custom network no container uses?".to_string()))
                    </div>
                    match rows {
                        Err(e) => {
                            <section class="empty small"><p>(e)</p></section>
                        }
                        Ok(rows) => {
                            <div class="table-wrap"><table class="table">
                                <thead><tr><th>"Network"</th><th>"Subnet"</th><th>"Used by"</th><th></th></tr></thead>
                                <tbody>
                                    for n in rows {
                                        <tr data-key=(n.id.clone())>
                                            <td>
                                                <span class="strong">(n.name.clone())</span>
                                                <div class="sub">
                                                    if let Some(s) = n.stack.clone() {
                                                        <a href=(format!("/hosts/{id}/stacks/{s}"))>(s)</a>
                                                        " · "
                                                    }
                                                    (n.driver.clone()) " · " (n.scope.clone())
                                                    if n.internal {
                                                        " · internal"
                                                    }
                                                    if n.builtin {
                                                        " · built in"
                                                    }
                                                </div>
                                            </td>
                                            <td class="muted">(if n.subnet.is_empty() { "—".to_string() } else { n.subnet.clone() })</td>
                                            <td>
                                                if n.builtin && n.used_by.is_empty() {
                                                    <span class="pill idle"><i></i>"system"</span>
                                                } else {
                                                    used_by(names: n.used_by.clone())
                                                }
                                            </td>
                                            <td><div class="row-actions">
                                                if !n.builtin {
                                                    api_button(url: format!("/api/hosts/{id}/resources/networks/remove"), label: "Remove network", glyph: Icon::Trash, tone: "danger", confirm: Some(format!("Remove network {}?", n.name)), body: Some(body(&n.id)))
                                                }
                                            </div></td>
                                        </tr>
                                    }
                                </tbody>
                            </table></div>
                        }
                    }
                </div>
            </div>
        )
    })
}
