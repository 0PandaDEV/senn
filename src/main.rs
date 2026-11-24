mod app;
mod components;
mod composables;
mod layouts;
mod pages;

#[path = "../shared/proto.rs"]
mod proto;
#[path = "../server/mod.rs"]
mod server;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    server::utils::config::init();
    server::utils::state::init().await;
    server::utils::notify::start().await;
    server::utils::metrics::start();

    let router = app::router().trusted_proxies(
        topcoat::router::TrustedProxies::new()
            .nearest(server::utils::config::config().trusted_proxy_hops),
    );
    topcoat::start(router.build()).await.unwrap();
}
