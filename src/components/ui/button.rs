use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

use super::icon::{Icon, icon};

#[component]
pub async fn job_button(
    cx: &Cx,
    #[into] url: String,
    #[into] title: String,
    #[into] label: String,
    glyph: Icon,
    #[default] confirm: Option<String>,
    #[default] redirect: Option<String>,
    #[default] tone: &str,
    #[default] text: bool,
) -> Result<impl View> {
    Ok(view! {
        <button type="button" class=(format!("button {tone}{}", if text { "" } else { " icon-only" })) data-job=(url) data-title=(title) data-confirm=(confirm) data-redirect=(redirect) title=(label.clone()) aria-label=(label.clone())>
            icon(name: glyph)
            if text {
                <span>(label)</span>
            }
        </button>
    })
}

#[component]
pub async fn api_button(
    cx: &Cx,
    #[into] url: String,
    #[into] label: String,
    glyph: Icon,
    #[default] confirm: Option<String>,
    #[default] body: Option<String>,
    #[default] tone: &str,
    #[default] text: bool,
) -> Result<impl View> {
    Ok(view! {
        <button type="button" class=(format!("button {tone}{}", if text { "" } else { " icon-only" })) data-api=(url) data-reload="" data-confirm=(confirm) data-body=(body) title=(label.clone()) aria-label=(label.clone())>
            icon(name: glyph)
            if text {
                <span>(label)</span>
            }
        </button>
    })
}
