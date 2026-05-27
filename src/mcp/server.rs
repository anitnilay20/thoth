//! MCP server entry point — wires up the ThothMcpServer and stdio transport.

use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

use super::tools::ThothMcpServer;
use super::state::ServerState;

/// Start the MCP server on stdio transport.
///
/// This function blocks until the client disconnects.
/// All diagnostic output goes to stderr — stdout is reserved for JSON-RPC.
pub async fn run_mcp_server() -> anyhow::Result<()> {
    // Route all tracing output to stderr so stdout stays clean for JSON-RPC.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Thoth MCP server");

    let state = ServerState::new();
    let server = ThothMcpServer::new(state);

    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    tracing::info!("Thoth MCP server stopped");
    Ok(())
}
