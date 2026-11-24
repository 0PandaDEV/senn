use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::{
        badges::{state_pill, svc_chip, usage},
        host_head::{host_head, update_check},
        ui::{
            button::job_button,
            icon::{Icon, icon},
            nav::nav,
            policy::policy_select,
        },
    },
    composables::use_host::{Usage, load, stack_issues, stack_summary},
    layouts::shell::shell,
};

fn stack_tone(running: usize, total: usize) -> (&'static str, &'static str) {
    match (running, total) {
        (_, 0) => ("idle", "not deployed"),
        (r, t) if r == t => ("ok", "running"),
        (0, _) => ("bad", "stopped"),
        _ => ("warn", "partial"),
    }
}

#[page("/hosts/{agent}/stacks", with(meta("Stacks · Senn")), with(PageMeta { public: false }))]
pub async fn host(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let stacks = host
        .state
        .as_ref()
        .map(|s| s.stacks.clone())
        .unwrap_or_default();
    let unmanaged = host.unmanaged().into_iter().cloned().collect::<Vec<_>>();
    let ready = host.online && host.state.as_ref().is_some_and(|s| s.stacks_dir_ok);
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "stacks",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                if ready {
                    <div data-live="stacks">
                    <div class="toolbar">
                        <h2>"Stacks"</h2>
                        <div class="inline">
                            update_check(host: &host)
                            <button type="button" class="button primary" data-open="#new-stack">
                                icon(name: Icon::Plus)
                                "New stack"
                            </button>
                        </div>
                    </div>
                    if stacks.is_empty() {
                        <section class="empty small">
                            <p>"No stack folders found in " <code>(host.state.as_ref().map(|s| s.stacks_dir.clone()).unwrap_or_default())</code> ". A stack is a subfolder with a compose file."</p>
                        </section>
                    } else {
                        <div class="table-wrap"><table class="table" data-minor="3 4">
                            <thead>
                                <tr><th>"Stack"</th><th>"Status"</th><th>"Usage"</th><th>"Services"</th><th>"Updates"</th><th></th></tr>
                            </thead>
                            <tbody>
                                for s in stacks {
                                    let containers = host.stack_containers(&s.name);
                                    let (running, total) = stack_summary(&containers);
                                    let (tone, label) = stack_tone(running, total);
                                    let used = Usage::sum(&containers);
                                    let issues = stack_issues(&containers);
                                    let updates = host.stack_updates(&s.name);
                                    let base = format!("/api/hosts/{id}/stacks/{}", s.name);
                                    <tr class="row">
                                        <td>
                                            <a class="strong go" (nav(cx, format!("/hosts/{id}/stacks/{}", s.name)))>(s.name.clone())</a>
                                            if let Some(e) = s.error.clone() {
                                                <div class="sub err" title=(e.clone())>"compose file has errors"</div>
                                            } else {
                                                <div class="sub" title=(host.stack_dir(&s.name))>(host.stack_dir(&s.name))</div>
                                            }
                                        </td>
                                        <td><div class="status">
                                            <span class=(format!("pill {tone}"))><i></i>(label) if total > 0 { " " (running) "/" (total) }</span>
                                            if issues > 0 {
                                                <span class="pill bad"><i></i>(issues) " unhealthy"</span>
                                            }
                                        </div></td>
                                        <td>usage(u: used)</td>
                                        <td><div class="chips">
                                            for svc in s.services.iter().take(4).cloned() {
                                                let found = containers.iter().find(|c| c.service.as_deref() == Some(svc.as_str()));
                                                let state = found.map(|c| c.state.clone()).unwrap_or_else(|| "none".into());
                                                let update = found.is_some_and(|c| host.has_update(c));
                                                svc_chip(name: svc, state: state.as_str(), update: update)
                                            }
                                            if s.services.len() > 4 {
                                                <span class="chip idle" title=(s.services[4..].join("\n"))>(format!("+{}", s.services.len() - 4))</span>
                                            }
                                        </div></td>
                                        <td>policy_select(agent: id.as_str(), scope: s.name.clone(), current: host.record.config.policies.get(&s.name).cloned().unwrap_or_default(), inherit: format!("Default ({})", host.record.config.policy_default()))</td>
                                        <td><div class="row-actions">
                                            if running == total && total > 0 {
                                                job_button(url: format!("{base}/stop"), title: format!("{} · stop", s.name), label: "Stop", glyph: Icon::Stop)
                                            } else {
                                                job_button(url: format!("{base}/up"), title: format!("{} · up", s.name), label: "Up", glyph: Icon::Play)
                                            }
                                            job_button(url: format!("{base}/restart"), title: format!("{} · restart", s.name), label: "Restart", glyph: Icon::Restart)
                                            job_button(url: format!("{base}/update"), title: format!("{} · update", s.name), label: "Pull and recreate", glyph: Icon::Update, tone: if updates > 0 { "warn" } else { "" })
                                            job_button(url: format!("{base}/down"), title: format!("{} · down", s.name), label: "Down", glyph: Icon::Power, tone: "danger", confirm: Some(format!("Stop and remove the containers of {}? Volumes are kept.", s.name)))
                                        </div></td>
                                    </tr>
                                }
                            </tbody>
                        </table></div>
                    }
                    if !unmanaged.is_empty() {
                        <div class="toolbar"><h2>"Other containers"</h2><span class="muted">"not part of a stack in the folder"</span></div>
                        <div class="table-wrap"><table class="table" data-minor="3">
                            <thead><tr><th>"Name"</th><th>"State"</th><th>"Usage"</th></tr></thead>
                            <tbody>
                                for c in unmanaged {
                                    <tr class="row">
                                        <td>
                                            <a class="strong go" (nav(cx, format!("/hosts/{id}/containers/{}", c.id)))>(c.name.clone())</a>
                                            <div class="sub" title=(c.image.clone())>(c.image.clone())</div>
                                        </td>
                                        <td>state_pill(state: &c.state, health: c.health.as_deref())</td>
                                        <td>usage(u: Usage::of(&c))</td>
                                    </tr>
                                }
                            </tbody>
                        </table></div>
                    }
                    </div>
                    <dialog id="new-stack" class="dialog wide">
                        <form data-stack-create=(id.clone())>
                            <h2>"New stack"</h2>
                            <p class="muted small">"Saved to " <code data-path-preview=(format!("{}/", host.stacks_dir()))>(format!("{}/…/docker-compose.yml", host.stacks_dir()))</code></p>
                            <label class="field">
                                <span>"Name"</span>
                                <input name="name" placeholder="my-app" required=(true)>
                            </label>
                            <label class="field">
                                <span>"docker-compose.yml"</span>
                                <textarea name="compose" rows="12" spellcheck="false" required=(true)>"services:\n  app:\n    image: nginx:alpine\n    ports:\n      - \"8080:80\"\n    restart: unless-stopped\n"</textarea>
                            </label>
                            <div class="dialog-actions">
                                <button type="button" class="button" data-close="">"Cancel"</button>
                                <button type="submit" class="button primary">"Create"</button>
                            </div>
                        </form>
                    </dialog>
                }
            </div>
        )
    })
}
