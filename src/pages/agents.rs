use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::ui::{
        button::job_button,
        icon::{Icon, icon},
    },
    layouts::shell::shell,
    server::utils::{format::ago, hub, release, state},
};

fn next_run(expr: &str) -> String {
    let Ok(cron) = crate::cron::Cron::parse(expr) else {
        return "invalid schedule".into();
    };
    let now = jiff::ZonedDateTime::now();
    match cron.next_after(&now) {
        Some(next) if next.date() == now.date() => format!("next {}", next.strftime("%H:%M")),
        Some(next) => format!("next {}", next.strftime("%b %-d %H:%M")),
        None => "never runs".into(),
    }
}

#[page("/agents", with(meta("Agents · Senn")), with(PageMeta { public: false }))]
pub async fn agents(cx: &Cx) -> Result<impl View> {
    let now = state::now();
    let list = state::agents().await;
    let releases = release::list().await;
    Ok(view! {
        shell(section: "agents",
            <div>
                <header class="page-head">
                    <div class="title">
                        <h1>"Agents"</h1>
                        <p class="sub">"Each agent is a small binary on a host. It connects out to this panel, so the host needs no open port."</p>
                    </div>
                    <button type="button" class="button primary" data-open="#add-agent">
                        icon(name: Icon::Plus)
                        "Add agent"
                    </button>
                </header>
                <div data-live="agents">
                if list.is_empty() {
                    <section class="empty">
                        <h2>"No agents"</h2>
                        <p>"Add one for every host you want to manage, including the machine this panel runs on."</p>
                    </section>
                } else {
                    <div class="table-wrap"><table class="table" data-minor="3 4">
                        <thead>
                            <tr><th>"Name"</th><th>"Status"</th><th>"Host"</th><th>"Updates"</th><th></th></tr>
                        </thead>
                        <tbody>
                            for a in list {
                                let online = hub::online(&a.id);
                                let target = release::resolve(&releases, &a.config.channel);
                                let running = a.info.as_ref().map(|i| i.version.clone()).unwrap_or_default();
                                let behind = target.clone().filter(|t| !running.is_empty() && release::version_of(t) != running);
                                <tr>
                                    <td><strong>(a.name.clone())</strong></td>
                                    <td>
                                        if online {
                                            <span class="pill ok"><i></i>"online"</span>
                                        } else if let Some(t) = a.last_seen {
                                            <span class="pill bad"><i></i>"offline · " (ago(now - t))</span>
                                        } else {
                                            <span class="pill idle"><i></i>"waiting"</span>
                                        }
                                    </td>
                                    <td class="muted">
                                        if let Some(info) = a.info.clone() {
                                            (info.hostname) " · agent " (info.version)
                                            if let Some(d) = info.docker {
                                                " · docker " (d)
                                            }
                                            if let Some(t) = behind.clone() {
                                                " "
                                                <span class="pill warn" title=(format!("{} is available", t))><i></i>(release::version_of(&t).to_string())</span>
                                            }
                                        } else {
                                            "—"
                                        }
                                    </td>
                                    <td class="muted">
                                        "images: " (a.config.update_cron.clone()) " · " (next_run(&a.config.update_cron)) " · "
                                        match a.config.policy_default() {
                                            "auto" => "updates automatically",
                                            "off" => "updates ignored",
                                            _ => "notifies only",
                                        }
                                        <div class="sub">
                                            "agent: " (release::label(&a.config.channel))
                                            if a.config.self_update {
                                                " · " (a.config.self_update_cron.clone()) " · " (next_run(&a.config.self_update_cron))
                                            } else {
                                                " · manual"
                                            }
                                        </div>
                                    </td>
                                    <td><div class="row-actions">
                                        if online && behind.is_some() {
                                            job_button(url: format!("/api/agents/{}/upgrade", a.id), title: format!("Update agent {}", a.name), label: "Update", glyph: Icon::Update, text: true)
                                        }
                                        <button type="button" class="button" data-edit-agent=(a.id.clone()) data-channel=(a.config.channel.clone()) data-self-update=(if a.config.self_update { "on" } else { "off" }) data-name=(a.name.clone()) data-policy=(a.config.policy_default()) data-update-cron=(a.config.update_cron.clone()) data-self-update-cron=(a.config.self_update_cron.clone()) data-address=(a.address.clone().unwrap_or_default()) data-detected=(a.info.as_ref().and_then(|i| i.address.clone()).unwrap_or_default())>
                                            icon(name: Icon::Pencil)
                                            <span>"Edit"</span>
                                        </button>
                                        <button type="button" class="button" data-token-agent=(a.id.clone())>
                                            icon(name: Icon::Refresh)
                                            <span>"New token"</span>
                                        </button>
                                        <button type="button" class="button danger icon-only" data-api=(format!("/api/agents/{}/delete", a.id)) data-reload="" data-confirm=(format!("Remove agent {}? The host keeps running, it just loses its connection to this panel.", a.name))>
                                            icon(name: Icon::Trash)
                                        </button>
                                    </div></td>
                                </tr>
                            }
                        </tbody>
                    </table></div>
                }
                </div>

                <dialog id="add-agent" class="dialog">
                    <form data-agent-create="">
                        <h2>"Add agent"</h2>
                        <label class="field">
                            <span>"Name"</span>
                            <input name="name" placeholder="my-server" required=(true)>
                        </label>
                        <div class="dialog-actions">
                            <button type="button" class="button" data-close="">"Cancel"</button>
                            <button type="submit" class="button primary">"Create"</button>
                        </div>
                    </form>
                </dialog>

                <dialog id="edit-agent" class="dialog">
                    <form data-agent-edit="">
                        <h2>"Agent settings"</h2>
                        <input type="hidden" name="id">
                        <label class="field">
                            <span>"Name"</span>
                            <input name="name" required=(true)>
                        </label>
                        <label class="field">
                            <span>"Address for port links " <small>"optional, host name or IP your browser can reach"</small></span>
                            <input name="address" placeholder="detected automatically" autocomplete="off">
                        </label>
                        <div class="dialog-section">
                            <h3>"Agent"</h3>
                            <p>"The senn-agent binary on this host."</p>
                        </div>
                        <label class="field">
                            <span>"Version"</span>
                            <select name="channel" data-select="">
                                <option value="stable">"Latest release"</option>
                                <option value="nightly">"Latest nightly"</option>
                                for r in releases.iter().take(40) {
                                    <option value=(r.tag.clone())>(format!("Pin to {}", r.tag))</option>
                                }
                            </select>
                        </label>
                        <label class="field">
                            <span>"Install agent updates"</span>
                            <select name="selfUpdate" data-select="">
                                <option value="on">"Automatically"</option>
                                <option value="off">"Manually, with the Update button"</option>
                            </select>
                        </label>
                        <label class="field">
                            <span>"Agent update schedule " <small>"cron, server time"</small></span>
                            <input name="selfUpdateCron" data-cron="" placeholder="0 4 * * *" autocomplete="off" spellcheck="false" required=(true)>
                        </label>
                        <div class="dialog-section">
                            <h3>"Containers"</h3>
                            <p>"Image updates for the stacks on this host. Single stacks and services can override this."</p>
                        </div>
                        <label class="field">
                            <span>"When a newer image is found"</span>
                            <select name="defaultPolicy" data-select="">
                                <option value="notify">"Notify only"</option>
                                <option value="auto">"Pull and recreate automatically"</option>
                                <option value="off">"Ignore (no checks, no notifications)"</option>
                            </select>
                        </label>
                        <label class="field">
                            <span>"Image check schedule " <small>"cron, host time"</small></span>
                            <input name="updateCron" data-cron="" placeholder="0 */6 * * *" autocomplete="off" spellcheck="false" required=(true)>
                        </label>
                        <div class="dialog-actions">
                            <button type="button" class="button" data-close="">"Cancel"</button>
                            <button type="submit" class="button primary">"Save"</button>
                        </div>
                    </form>
                </dialog>

                <dialog id="install-agent" class="dialog wide">
                    <h2>"Install the agent"</h2>
                    <p class="muted">"Run this on the host. The token is shown once."</p>
                    <pre class="code" id="install-cmd"></pre>
                    <div class="dialog-actions">
                        <button type="button" class="button" data-copy="#install-cmd">"Copy"</button>
                        <button type="button" class="button primary" data-close="" data-reload="">"Done"</button>
                    </div>
                </dialog>
            </div>
        )
    })
}
