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
                    <img class="brand-logo" src="/logo.svg" width="34" height="28" alt="">
                    <h1>"Senn"</h1>
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
