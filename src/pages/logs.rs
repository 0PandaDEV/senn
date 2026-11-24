use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::{badges::state_pill, host_head::host_head},
    composables::use_host::load,
    layouts::shell::shell,
    proto::ContainerInfo,
    server::utils::http_util::query_map,
};

#[page("/hosts/{agent}/logs", with(meta("Logs · Senn")), with(PageMeta { public: false }))]
pub async fn logs(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let all: Vec<ContainerInfo> = host.state.as_ref().map(|s| s.containers.clone()).unwrap_or_default();
    let mut groups: Vec<(String, Vec<ContainerInfo>)> = Vec::new();
    for c in &all {
        let g = c.stack.clone().unwrap_or_default();
        match groups.iter_mut().find(|(name, _)| *name == g) {
            Some((_, list)) => list.push(c.clone()),
            None => groups.push((g, vec![c.clone()])),
        }
    }
    groups.sort_by(|a, b| (a.0.is_empty(), &a.0).cmp(&(b.0.is_empty(), &b.0)));
    let wanted: Vec<String> = query_map(cx)
        .get("c")
        .map(|c| c.split(',').filter(|s| all.iter().any(|x| x.id == *s)).map(String::from).collect())
        .unwrap_or_default();
    let selected: Vec<String> = if wanted.is_empty() {
        groups.first().map(|(_, list)| list.iter().map(|c| c.id.clone()).collect()).unwrap_or_default()
    } else {
        wanted
    };
    let url = (!selected.is_empty()).then(|| format!("/api/hosts/{id}/logs?containers={}", selected.join(",")));
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "logs",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                <div class="logs-page">
                    <aside class="log-picker" data-log-picker=(id.clone())>
                        if all.is_empty() {
                            <p class="muted small">"No containers on this host."</p>
                        }
                        for (group, list) in groups {
                            <div class="log-group">
                                <button type="button" class="log-group-head" data-log-group="">
                                    (if group.is_empty() { "Other containers".to_string() } else { group.clone() })
                                </button>
                                for c in list {
                                    <label class="log-item">
                                        <input type="checkbox" value=(c.id.clone()) checked=(selected.contains(&c.id))>
                                        <span class="log-item-name">(c.service.clone().unwrap_or_else(|| c.name.clone()))</span>
                                        state_pill(state: &c.state, health: c.health.as_deref())
                                    </label>
                                }
                            </div>
                        }
                    </aside>
                    <div class="log-view" data-log-view="">
                        if let Some(u) = url {
                            <div data-logs=(u) data-multi=""></div>
                        } else {
                            <section class="empty small"><p>"Pick containers on the left."</p></section>
                        }
                    </div>
                </div>
            </div>
        )
    })
}
