//! Workshop backend: the only component holding bitcoind credentials.
//! Faucet, chain clock (auto-miner, time jumps) and the browser event bus.

mod api;
mod chain;
mod config;
mod events;
mod registry;
mod room;
mod rpc;

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::broadcast;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{api::AppState, chain::MinerSettings, config::Config, rpc::Rpc};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let node = Rpc::new(&config.rpc_url, &config.rpc_user, &config.rpc_pass);
    let miner =
        MinerSettings { paused: config.miner_paused, interval_s: config.miner_interval.as_secs() };
    let (events, _) = broadcast::channel(256);
    let chain =
        chain::Chain::bootstrap(node, &config.wallet, config.seed_blocks, miner, events).await?;

    tokio::spawn(chain.clone().run_miner());
    tokio::spawn(chain.clone().run_watcher());

    let registry = registry::Registry::load(config.registry_path.clone())?;
    let listen = config.listen;
    let state = Arc::new(AppState::new(config, chain, registry));
    tokio::spawn(events::record_feed(state.clone()));
    let app = api::router(state).layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(listen).await?;
    info!(%listen, "workshop-server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
