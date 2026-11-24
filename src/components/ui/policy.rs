use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

#[component]
pub async fn policy_select(
    cx: &Cx,
    #[into] agent: String,
    #[into] scope: String,
    #[into] current: String,
    #[into] inherit: String,
) -> Result<impl View> {
    Ok(view! {
        <div class="cell-select">
            <select data-policy=(format!("/api/hosts/{agent}/policy")) data-scope=(scope) data-select="ghost" aria-label="Update policy">
                <option value="" selected=(current.is_empty())>(inherit)</option>
                <option value="notify" selected=(current == "notify")>"Notify only"</option>
                <option value="auto" selected=(current == "auto")>"Auto update"</option>
                <option value="off" selected=(current == "off")>"Off (ignore)"</option>
            </select>
        </div>
    })
}
