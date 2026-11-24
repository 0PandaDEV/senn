use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

#[component]
pub async fn used_by(cx: &Cx, names: Vec<String>) -> Result<impl View> {
    let shown: Vec<String> = names.iter().take(2).cloned().collect();
    let rest = names.len().saturating_sub(2);
    let title = names.join("\n");
    Ok(view! {
        if names.is_empty() {
            <span class="pill idle"><i></i>"unused"</span>
        } else {
            <div class="chips" title=(title)>
                for n in shown {
                    <span class="chip idle">(n)</span>
                }
                if rest > 0 {
                    <span class="chip idle">(format!("+{rest}"))</span>
                }
            </div>
        }
    })
}
