mod server;
mod types;

use std::sync::Arc;

use gleanmark_core::models::Config;
use gleanmark_core::GleanMark;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Log to stderr (stdout is reserved for JSON-RPC)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let config = Config::default();
    let gm = Arc::new(GleanMark::new(config).await?);

    let mcp_server = server::GleanMarkMcp::new(gm);
    let transport = rmcp::transport::io::stdio();

    let service = mcp_server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
