//! # Tandoor MCP Server
//!
//! A Model Context Protocol (MCP) server that provides tools for interacting with Tandoor,
//! a recipe management system. This server allows AI assistants to search recipes, create
//! new recipes, manage shopping lists, and more through a standardized protocol.
//!
//! ## Environment Variables
//!
//! - `TANDOOR_BASE_URL`: Tandoor server URL (default: http://localhost:8080)
//! - `TANDOOR_USERNAME`: Tandoor username for authentication (default: admin)
//! - `TANDOOR_PASSWORD`: Tandoor password for authentication (default: admin)
//! - `TANDOOR_AUTH_TOKEN`: Pre-set auth token to bypass username/password auth (avoids rate limiting)
//! - `RUST_LOG`: Logging level (info, debug, trace, etc.)
//!
//! ## Usage
//!
//! ```bash
//! TANDOOR_BASE_URL=http://your-tandoor-server:8080 \
//! TANDOOR_USERNAME=your_username \
//! TANDOOR_PASSWORD=your_password \
//! cargo run
//! ```

use mcp_tandoor::server::TandoorMcpServer;
use rmcp::ServiceExt;
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (silently ignored if not found)
    let _ = dotenvy::dotenv();

    // Initialize tracing — MUST write to stderr since stdout is the MCP transport
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    // Get configuration from environment variables
    let base_url =
        env::var("TANDOOR_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let username = env::var("TANDOOR_USERNAME").unwrap_or_else(|_| "admin".to_string());

    let password = env::var("TANDOOR_PASSWORD").unwrap_or_else(|_| "admin".to_string());

    // Create server and authenticate
    tracing::info!("Validating Tandoor credentials...");
    let server = TandoorMcpServer::new_with_credentials(
        base_url.clone(),
        username.clone(),
        password.clone(),
    );

    if let Ok(token) = env::var("TANDOOR_AUTH_TOKEN") {
        tracing::info!("Using pre-set token from TANDOOR_AUTH_TOKEN");
        server
            .set_global_auth_token(token)
            .await
            .expect("Failed to set auth token");
    } else if let Err(e) = server
        .authenticate(username.clone(), password.clone())
        .await
    {
        tracing::error!("Authentication failed: {}", e);
        tracing::error!("Please verify:");
        tracing::error!("  - TANDOOR_BASE_URL is correct: {}", base_url);
        tracing::error!("  - TANDOOR_USERNAME is correct: {}", username);
        tracing::error!("  - TANDOOR_PASSWORD is correct");
        tracing::error!("  - Tandoor server is running and accessible");
        std::process::exit(1);
    }

    // Test that we can actually use the token
    tracing::info!("Testing API access with token...");
    match server.test_api_access().await {
        Ok(_) => {
            tracing::info!("API access test passed");
        }
        Err(e) => {
            tracing::warn!("API access test failed: {}", e);
            tracing::warn!("The server will continue, but some functionality may be limited.");
        }
    }

    tracing::info!("Starting Tandoor MCP Server on stdio transport");

    // Run server over stdio (stdin/stdout)
    let service = server.serve(rmcp::transport::io::stdio()).await?;

    // Wait until the client disconnects
    service.waiting().await?;

    tracing::info!("Tandoor MCP Server shutting down");

    Ok(())
}
