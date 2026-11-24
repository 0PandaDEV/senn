use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::app::{PageMeta, meta};

#[page("/setup", with(meta("Set up · Senn")), with(PageMeta { public: true }))]
pub async fn setup(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <div class="login">
            <form class="login-card" data-api="/api/auth/setup" data-redirect="/">
                <div class="login-brand">
                    <svg width="28" height="28" viewBox="0 0 32 32" aria-hidden="true"><path d="M6 24 13 9l4 8 3-5 6 12z" fill="currentColor"></path></svg>
                    <h1>"senn"</h1>
                </div>
                <p class="muted">"Create the account for this instance. It is the only one, there is no user management."</p>
                <label class="field">
                    <span>"Username"</span>
                    <input name="username" autocomplete="username" autofocus=(true) required=(true)>
                </label>
                <label class="field">
                    <span>"Password"</span>
                    <input type="password" name="password" autocomplete="new-password" minlength="10" required=(true)>
                </label>
                <label class="field">
                    <span>"Repeat password"</span>
                    <input type="password" name="confirm" autocomplete="new-password" required=(true)>
                </label>
                <button type="submit" class="button primary wide">"Create account"</button>
            </form>
        </div>
    })
}
