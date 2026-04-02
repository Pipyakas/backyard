mod cli;
mod state;
mod orchestrator;
mod server;
mod models;

use clap::Parser;
use std::sync::Arc;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    
    // Initialize shared state
    let app_state = Arc::new(state::State::init()?);

    if cli.command.is_some() {
        cli::handle_command(cli, &app_state).await?;
    } else {
        // Default: Start web server
        server::start_server(app_state).await?;
    }

    Ok(())
}
