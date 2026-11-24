use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

use crate::server::utils::metrics;

#[component]
pub async fn metrics_panel(
    cx: &Cx,
    kind: &str,
    host: &str,
    #[default] name: &str,
) -> Result<impl View> {
    let _ = cx;
    Ok(view! {
        if metrics::enabled() {
            <section class="metrics" data-metrics=(kind.to_string()) data-host=(host.to_string()) data-name=(name.to_string())></section>
        } else {
            <section class="empty small">
                <p>"Metrics storage is not configured. Set " <code>"SENN_METRICS_URL"</code> " to a VictoriaMetrics instance."</p>
            </section>
        }
    })
}
