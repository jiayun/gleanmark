use std::sync::Arc;

use gleanmark_core::models::Config;
use gleanmark_core::GleanMark;
use gleanmark_server::build_router;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::load();
    let gleanmark = GleanMark::new(config).await?;
    let app = build_router(Arc::new(gleanmark));

    let addr = "127.0.0.1:21580";
    info!("Starting server at http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
