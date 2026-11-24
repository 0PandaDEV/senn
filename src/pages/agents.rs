use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::ui::icon::{Icon, icon},
    layouts::shell::shell,
    server::utils::{format::ago, hub, state},
};

#[page("/agents", with(meta("Agents · Senn")), with(PageMeta { public: false }))]
pub async fn agents(cx: &Cx) -> Result<impl View> {
    let now = state::now();
    let list = state::agents().await;
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
                                        } else {
                                            "—"
                                        }
                                    </td>
                                    <td class="muted">
                                        if a.config.auto_update {
                                            "auto every " (a.config.update_interval_secs / 60) " min"
                                        } else {
                                            "checks only"
                                        }
                                    </td>
                                    <td><div class="row-actions">
                                        <button type="button" class="button" data-edit-agent=(a.id.clone()) data-name=(a.name.clone()) data-auto=(a.config.auto_update) data-interval=(a.config.update_interval_secs / 60) data-address=(a.address.clone().unwrap_or_default()) data-detected=(a.info.as_ref().and_then(|i| i.address.clone()).unwrap_or_default())>
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
                            <input name="name" placeholder="panda-02" required=(true)>
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
                        <label class="check">
                            <input type="checkbox" name="autoUpdate">
                            <span>"Apply updates automatically for services labelled " <code>"senn.update=auto"</code></span>
                        </label>
                        <label class="field">
                            <span>"Check for image updates every (minutes)"</span>
                            <input type="number" name="updateIntervalMinutes" min="5" max="43200" required=(true)>
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
