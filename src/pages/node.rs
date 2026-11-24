use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::{
        host_head::host_head,
        metrics_panel::metrics_panel,
        node_card::{cards, node_card},
    },
    composables::use_host::load,
    layouts::shell::shell,
};

#[page("/hosts/{agent}", with(meta("Host · Senn")), with(PageMeta { public: false }))]
pub async fn node(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let card = cards().await.into_iter().find(|c| c.id == id);
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "metrics",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                <div data-live="node">
                    if let Some(c) = card {
                        <div class="host-card static">
                            node_card(c: c, bare: true)
                        </div>
                    }
                </div>
                metrics_panel(kind: "host", host: id.as_str())
            </div>
        )
    })
}
