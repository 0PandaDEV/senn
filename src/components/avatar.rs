use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

use crate::server::utils::state::Profile;

#[component]
pub async fn avatar(cx: &Cx, profile: Profile, #[default] large: bool) -> Result<impl View> {
    let class = if large { "avatar large" } else { "avatar" };
    Ok(view! {
        if let Some(v) = profile.avatar_version {
            <img class=(class) src=(format!("/api/account/avatar?v={v}")) alt="">
        } else {
            <span class=(class) aria-hidden="true">(profile.initials())</span>
        }
    })
}
