mod state;
mod server;
mod bench_http;
mod harness;
mod worker;
mod auth;

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state = Arc::new(state::State::init()?);
    server::start_server(state).await
}
