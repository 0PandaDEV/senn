use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::host_head::host_head,
    composables::use_host::load,
    layouts::shell::shell,
};

#[page("/hosts/{agent}/files", with(meta("Files · Senn")), with(PageMeta { public: false }))]
pub async fn files(cx: &Cx) -> Result<impl View> {
    let host = load(cx).await?;
    let id = host.record.id.clone();
    let dir = host.stacks_dir();
    Ok(view! {
        shell(section: "host", host: id.as_str(), tab: "files",
            <div data-agent=(id.clone())>
                host_head(host: &host)
                if host.online {
                    <p class="muted path-note">"Everything inside " <code>(dir.clone())</code> " on this host."</p>
                    <div data-files="" data-agent=(id.clone()) data-root="" data-abs=(dir.clone())></div>
                }
            </div>
        )
    })
}
