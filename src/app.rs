use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::{Cx, try_context},
    cookie::CookieLayer,
    font::{Font, font, link},
    metadata::Metadata,
    router::{
        Router, RouterBuilder, RouterBuilderDirectoryExt, RouterBuilderDiscoverExt, Slot, layout,
    },
    runtime::RuntimeLayer,
    view::{View, view},
};

use crate::components::{jobs::jobs_panel, ui::icon::sprite};

pub struct PageMeta {
    pub public: bool,
}

const COMMIT_MONO: Font = font! {
    "Commit Mono",
    @font-face {
        src: url("/fonts/commit-mono.woff2") format("woff2") tech("variations");
        font-weight: 200 700;
        font-display: swap;
    }
};

const STYLES: [&str; 7] = [
    "base",
    "layout",
    "components",
    "editor",
    "files",
    "logs",
    "metrics",
];

const SCRIPTS: [&str; 21] = [
    "useLive",
    "useToast",
    "useDialog",
    "useApi",
    "useJob",
    "useLogs",
    "useTabs",
    "useFiles",
    "useEnv",
    "useAgents",
    "useMotion",
    "useMenu",
    "useFit",
    "useHost",
    "useSelect",
    "useCron",
    "useHostCard",
    "usePolicy",
    "useFade",
    "useSettings",
    "useMetrics",
];

pub fn meta(title: &'static str) -> Metadata {
    Metadata::new()
        .title(title)
        .viewport("width=device-width, initial-scale=1")
        .theme_color("#0e0e0f")
}

#[layout("/", with(Metadata::new().title("Senn").viewport("width=device-width, initial-scale=1")))]
async fn root(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    let _ = try_context::<PageMeta>(cx);
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                topcoat::metadata::tags()
                <link rel="icon" type="image/svg+xml" href="/favicon.svg">
                link(font: COMMIT_MONO)
                for style in STYLES {
                    <link rel="stylesheet" href=(format!("/styles/{style}.css"))>
                }
                topcoat::runtime::script()
                for script in SCRIPTS {
                    <script type="module" src=(format!("/js/{script}.js"))></script>
                }
            </head>
            <body>
                sprite()
                <div class="toasts" id="toasts"></div>
                (slot)
                jobs_panel()
            </body>
        </html>
    })
}

pub fn router() -> RouterBuilder {
    Router::builder()
        .discover()
        .layer(CookieLayer::new())
        .serve_dir("/styles/{*file}", "./styles")
        .public_dir("./public")
        .assets(
            AssetBundle::load()
                .expect("asset bundle missing: run with `topcoat dev` or `topcoat asset bundle`"),
        )
        .layer(RuntimeLayer::new())
}
