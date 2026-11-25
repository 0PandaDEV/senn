use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::{
        avatar::avatar,
        ui::icon::{Icon, icon},
    },
    layouts::shell::shell,
    server::utils::{notify, state},
};

const EVENTS: [(&str, &str, &str); 8] = [
    (
        "agent_offline",
        "Agent offline",
        "an agent stays disconnected for 30 seconds",
    ),
    (
        "agent_online",
        "Agent back online",
        "an agent that was reported offline reconnects",
    ),
    (
        "container_crash",
        "Container crashed",
        "a container exits with an error code or runs out of memory",
    ),
    (
        "container_unhealthy",
        "Container unhealthy",
        "a health check starts failing",
    ),
    (
        "updates",
        "Image updates",
        "an update check finds new images",
    ),
    (
        "auto_update",
        "Auto updates",
        "a service was updated automatically, or failed to",
    ),
    (
        "job_failed",
        "Action failed",
        "an action run from the panel fails",
    ),
    (
        "job_done",
        "Action finished",
        "an action run from the panel succeeds",
    ),
];

#[page("/settings", with(meta("Settings · Senn")), with(PageMeta { public: false }))]
pub async fn settings(cx: &Cx) -> Result<impl View> {
    let cfg = notify::current();
    let me = state::profile().await;
    let on = |key: &str| match key {
        "agent_offline" => cfg.agent_offline,
        "agent_online" => cfg.agent_online,
        "container_crash" => cfg.container_crash,
        "container_unhealthy" => cfg.container_unhealthy,
        "updates" => cfg.updates,
        "auto_update" => cfg.auto_update,
        "job_failed" => cfg.job_failed,
        "job_done" => cfg.job_done,
        _ => false,
    };
    let rows: Vec<(&str, &str, &str, bool)> =
        EVENTS.iter().map(|(k, l, d)| (*k, *l, *d, on(k))).collect();
    Ok(view! {
        shell(section: "settings",
            <div class="settings-page">
                <header class="page-head">
                    <div class="title">
                        <h1>"Settings"</h1>
                    </div>
                    <button type="submit" form="settings-form" class="button primary">"Save"</button>
                </header>
                <div class="settings-layout">
                <div class="settings">
                <section class="settings-card account">
                    <header class="settings-head">
                        <div>
                            <h2>"Account"</h2>
                            <p>"How you show up in the sidebar."</p>
                        </div>
                    </header>
                    <div class="settings-body">
                    <div class="account-row">
                        <div class="field">
                            <span>"Picture"</span>
                            <div class="account-avatar" data-avatar="">
                                avatar(profile: me.clone())
                                <label class="button">
                                    "Upload"
                                    <input type="file" accept="image/png,image/jpeg,image/webp,image/gif" data-avatar-file="" hidden=(true)>
                                </label>
                                if me.avatar_version.is_some() {
                                    <button type="button" class="button" data-avatar-remove="">"Remove"</button>
                                }
                            </div>
                        </div>
                        <form class="account-name" data-api="/api/account/profile" data-success="Name saved" data-reload="">
                            <label class="field">
                                <span>"Display name " <small>(format!("signed in as {}", me.username))</small></span>
                                <input name="display_name" value=(me.display_name.clone().unwrap_or_default()) placeholder=(me.username.clone()) autocomplete="off">
                            </label>
                            <button type="submit" class="button">"Save name"</button>
                        </form>
                    </div>
                    <form class="account-password" data-api="/api/account/password" data-success="Password changed, other sessions were signed out" data-reload="">
                        <label class="field">
                            <span>"Current password"</span>
                            <input type="password" name="current" autocomplete="current-password" required=(true)>
                        </label>
                        <label class="field">
                            <span>"New password " <small>"at least 10 characters"</small></span>
                            <input type="password" name="password" autocomplete="new-password" minlength="10" required=(true)>
                        </label>
                        <label class="field">
                            <span>"Repeat new password"</span>
                            <input type="password" name="confirm" autocomplete="new-password" minlength="10" required=(true)>
                        </label>
                        <button type="submit" class="button">"Change password"</button>
                    </form>
                    </div>
                </section></div>
                <form id="settings-form" class="settings" data-api="/api/settings/notify" data-success="Notification settings saved" data-settings="">
                    <section class="settings-card">
                        <header class="settings-head">
                            <div>
                                <h2>"Notifications"</h2>
                                <p>"Where senn sends alerts. Every event you turn on below goes to all of these."</p>
                            </div>
                            <label class="switch" title="Send notifications">
                                <input type="checkbox" name="enabled" checked=(cfg.enabled)>
                                <span></span>
                            </label>
                        </header>
                        <div class="settings-body">
                        <input type="hidden" name="targets" value=(cfg.targets.clone())>
                        <div class="targets" data-targets=""></div>
                        <div class="targets-actions">
                            <button type="button" class="button" data-target-add="">
                                icon(name: Icon::Plus)
                                "Add service"
                            </button>
                            <button type="button" class="button" data-target-test="">
                                icon(name: Icon::Info)
                                "Send test"
                            </button>
                        </div>
                        <details class="formats">
                            <summary>"Supported services and URL formats"</summary>
                            <dl>
                                for (name, example) in notify::SCHEMES.iter() {
                                    <dt>(*name)</dt>
                                    <dd>
                                        for part in example.split(" · ") {
                                            <code>(part.to_string())</code>
                                        }
                                    </dd>
                                }
                            </dl>
                        </details>
                        </div>
                    </section>
                    <section class="settings-card events-card">
                        <header class="settings-head">
                            <div>
                                <h2>"Events"</h2>
                                <p>"What you want to hear about."</p>
                            </div>
                        </header>
                        <div class="settings-body">
                        <div class="events">
                            for (key, label, desc, checked) in rows {
                                <label class="event">
                                    <span class="event-text"><b>(label)</b><small>(desc)</small></span>
                                    <span class="switch">
                                        <input type="checkbox" name=(key) checked=(checked)>
                                        <span></span>
                                    </span>
                                </label>
                            }
                        </div>
                        </div>
                    </section>
                </form>
                </div>
            </div>
        )
    })
}
