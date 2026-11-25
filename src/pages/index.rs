use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

use crate::{
    app::{PageMeta, meta},
    components::node_card::{cards, node_card, summary},
    components::ui::{
        icon::{Icon, icon},
        nav::nav,
    },
    layouts::shell::shell,
};

#[page("/", with(meta("Overview · Senn")), with(PageMeta { public: false }))]
pub async fn index(cx: &Cx) -> Result<impl View> {
    let cards = cards().await;
    let sum = summary(&cards);
    Ok(view! {
        shell(section: "overview",
            <div>
                <header class="page-head">
                    <div class="title"><h1>"Overview"</h1></div>
                </header>
                <div data-live="cards">
                if cards.is_empty() {
                    <section class="empty">
                        icon(name: Icon::Server, size: 28)
                        <h2>"Nothing here yet"</h2>
                        <p>"Senn manages Docker hosts through agents. Add an agent, run the install command on the host, and its stacks show up here."</p>
                        <a class="button primary" (nav(cx, "/agents#add-agent"))>
                            icon(name: Icon::Plus)
                            "Add your first agent"
                        </a>
                    </section>
                } else {
                    <div class="overview-stats">
                        <div class="stat-tile">
                            <span>"Hosts"</span>
                            <b>(sum.online) <small>" / " (sum.hosts) " online"</small></b>
                        </div>
                        <div class="stat-tile">
                            <span>"Containers"</span>
                            <b>(sum.running) <small>" / " (sum.total) " running"</small></b>
                        </div>
                        <div class="stat-tile">
                            <span>"Stacks"</span>
                            <b>(sum.stacks)</b>
                        </div>
                        <div class=(if sum.updates > 0 { "stat-tile warn" } else { "stat-tile" })>
                            <span>"Updates"</span>
                            <b>(sum.updates) <small>" available"</small></b>
                        </div>
                    </div>
                    <div class="grid">
                        for c in cards {
                            <a class="host-card" data-host-card=(c.id.clone()) (nav(cx, format!("/hosts/{}", c.id)))>
                                node_card(c: c)
                            </a>
                        }
                    </div>
                }
                </div>
            </div>
        )
    })
}
