use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::app::{PageMeta, meta};

#[page("/login", with(meta("Sign in · Senn")), with(PageMeta { public: true }))]
pub async fn login(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <div class="login">
            <form class="login-card" data-api="/api/auth/login" data-redirect="/">
                <div class="login-brand">
                    <svg width="28" height="28" viewBox="0 0 32 32" aria-hidden="true"><path d="M6 24 13 9l4 8 3-5 6 12z" fill="currentColor"></path></svg>
                    <h1>"senn"</h1>
                </div>
                <label class="field">
                    <span>"Username"</span>
                    <input name="username" autocomplete="username" autofocus=(true) required=(true)>
                </label>
                <label class="field">
                    <span>"Password"</span>
                    <input type="password" name="password" autocomplete="current-password" required=(true)>
                </label>
                <button type="submit" class="button primary wide">"Sign in"</button>
            </form>
        </div>
    })
}
