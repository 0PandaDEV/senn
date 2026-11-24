use topcoat::{
    Result,
    context::{Cx, try_context},
    view::{View, component, view},
};

use crate::{
    components::ui::icon::{Icon, icon},
    server::utils::{
        auth::{Authed, read_cookie},
        jobs,
    },
};

#[component]
pub async fn jobs_panel(cx: &Cx) -> Result<impl View> {
    let list = if try_context::<Authed>(cx).is_some() {
        jobs::visible()
    } else {
        Vec::new()
    };
    let wanted = read_cookie(cx, "senn_job_tab").and_then(|v| v.parse::<u64>().ok());
    let active = list
        .iter()
        .find(|j| Some(j.id) == wanted)
        .or(list.last())
        .map(|j| j.id);
    let collapsed = (read_cookie(cx, "senn_jobs_collapsed").as_deref() == Some("1")).then_some("");
    let empty = list.is_empty();
    Ok(view! {
        <div class="jobs" id="jobs" hidden=(empty) data-collapsed=(collapsed)>
            <div class="jobs-bar">
                <div class="jobs-tabs" id="jobs-tabs" role="tablist">
                    for j in list.clone() {
                        <div class="jobs-tab" data-job-id=(j.id.to_string()) data-key=(j.key.clone()) data-state=(j.state) data-active=((Some(j.id) == active).then_some(""))>
                            <button type="button" class="jobs-tab-main" role="tab" data-job-select=(j.id.to_string())>
                                <i class="jobs-dot"></i>
                                <span class="jobs-tab-title">(j.title.clone())</span>
                            </button>
                            <button type="button" class="jobs-tab-x" data-job-dismiss=(j.id.to_string()) aria-label="Close">icon(name: Icon::X, size: 12)</button>
                        </div>
                    }
                </div>
                <button type="button" class="icon-button" id="jobs-toggle" aria-label="Toggle output">icon(name: Icon::Chevron, size: 14)</button>
            </div>
            <div class="jobs-panes" id="jobs-panes">
                for j in list {
                    <pre class="jobs-body" data-job-id=(j.id.to_string()) hidden=(Some(j.id) != active)>(j.text.clone())</pre>
                }
            </div>
        </div>
    })
}
