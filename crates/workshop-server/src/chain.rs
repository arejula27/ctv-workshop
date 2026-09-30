//! Chain control: node bootstrap, mining, the auto-miner and the block watcher.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{Mutex, broadcast, watch};
use tracing::{info, warn};

use crate::{
    events::Event,
    rpc::{RPC_IN_WARMUP, RPC_WALLET_ALREADY_LOADED, RPC_WALLET_NOT_FOUND, Rpc},
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MinerSettings {
    pub paused: bool,
    pub interval_s: u64,
}

pub struct Chain {
    pub node: Rpc,
    pub wallet: Rpc,
    wallet_name: String,
    mining_addr: String,
    /// Serializes block generation so concurrent requests don't interleave.
    mine_lock: Mutex<()>,
    pub events: broadcast::Sender<Event>,
    pub miner: watch::Sender<MinerSettings>,
    /// Unix ms of the next auto-mined block, 0 when paused.
    next_block_at_ms: AtomicU64,
}

impl Chain {
    /// Waits for bitcoind, loads (or creates) the faucet wallet and mines the
    /// initial blocks.
    pub async fn bootstrap(
        node: Rpc,
        wallet_name: &str,
        seed_blocks: u64,
        miner: MinerSettings,
        events: broadcast::Sender<Event>,
    ) -> Result<Arc<Self>> {
        wait_for_node(&node).await?;
        ensure_wallet(&node, wallet_name).await?;
        let wallet = node.wallet(wallet_name);
        let mining_addr: String =
            wallet.call("getnewaddress", json!(["mining", "bech32m"])).await?;

        let chain = Arc::new(Self {
            node,
            wallet,
            wallet_name: wallet_name.to_owned(),
            mining_addr,
            mine_lock: Mutex::new(()),
            events,
            miner: watch::channel(miner).0,
            next_block_at_ms: AtomicU64::new(0),
        });

        let height = chain.height().await?;
        if height < seed_blocks {
            info!(from = height, to = seed_blocks, "seeding chain");
            chain.mine(seed_blocks - height).await?;
        }
        Ok(chain)
    }

    pub async fn height(&self) -> Result<u64> {
        Ok(self.node.call("getblockcount", json!([])).await?)
    }

    /// Mines `n` blocks to the faucet wallet and returns the new height.
    pub async fn mine(&self, n: u64) -> Result<u64> {
        let _guard = self.mine_lock.lock().await;
        // generatetoaddress is capped per call to keep responses small
        let mut left = n;
        while left > 0 {
            let batch = left.min(100);
            let _: Vec<String> =
                self.node.call("generatetoaddress", json!([batch, self.mining_addr])).await?;
            left -= batch;
        }
        // bitcoind only flushes its block index now and then; if it gets killed
        // (Docker Desktop crash, laptop sleep) it restarts from genesis and every
        // contract in the room is gone. gettxoutsetinfo forces a flush, and on a
        // workshop-sized regtest it takes milliseconds.
        if let Err(e) =
            self.node.call::<serde_json::Value>("gettxoutsetinfo", json!(["none"])).await
        {
            warn!("flush after mining failed: {e}");
        }
        self.height().await
    }

    pub async fn send(&self, address: &str, sats: u64) -> Result<String> {
        let btc = bitcoin::Amount::from_sat(sats).to_btc();
        match self.wallet.call("sendtoaddress", json!([address, btc])).await {
            // bitcoind restarted and the wallet isn't loaded yet: load it and retry
            Err(e) if e.code() == Some(RPC_WALLET_NOT_FOUND) => {
                warn!("faucet wallet not loaded, reloading");
                ensure_wallet(&self.node, &self.wallet_name).await?;
                Ok(self.wallet.call("sendtoaddress", json!([address, btc])).await?)
            }
            res => Ok(res?),
        }
    }

    pub fn next_block_in_ms(&self) -> Option<u64> {
        match self.next_block_at_ms.load(Ordering::Relaxed) {
            0 => None,
            at => Some(at.saturating_sub(now_ms())),
        }
    }

    /// Mines one block every `interval_s` unless paused. Settings changes
    /// restart the countdown.
    pub async fn run_miner(self: Arc<Self>) {
        let mut rx = self.miner.subscribe();
        loop {
            let settings = *rx.borrow_and_update();
            if settings.paused {
                self.next_block_at_ms.store(0, Ordering::Relaxed);
                if rx.changed().await.is_err() {
                    return;
                }
                continue;
            }
            let interval = Duration::from_secs(settings.interval_s);
            self.next_block_at_ms.store(now_ms() + interval.as_millis() as u64, Ordering::Relaxed);
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = self.mine(1).await {
                        warn!("auto-miner: {e:#}");
                    }
                }
                changed = rx.changed() => if changed.is_err() { return },
            }
        }
    }

    /// Emits a `block` event whenever the tip changes, whoever mined it.
    pub async fn run_watcher(self: Arc<Self>) {
        let mut last = String::new();
        loop {
            match self.node.call::<String>("getbestblockhash", json!([])).await {
                Ok(hash) if hash != last => {
                    if let Ok(height) = self.height().await {
                        let _ = self.events.send(Event::Block { height, hash: hash.clone() });
                    }
                    last = hash;
                }
                Ok(_) => {}
                Err(e) => warn!("watcher: {e}"),
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

async fn wait_for_node(node: &Rpc) -> Result<()> {
    for attempt in 1..=60 {
        match node.call::<serde_json::Value>("getblockchaininfo", json!([])).await {
            Ok(info) => {
                info!(chain = %info["chain"], blocks = %info["blocks"], "bitcoind ready");
                return Ok(());
            }
            Err(e) if e.code() == Some(RPC_IN_WARMUP) || e.code().is_none() => {
                if attempt % 10 == 0 {
                    info!("waiting for bitcoind: {e}");
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
    anyhow::bail!("bitcoind not reachable after 60s")
}

async fn ensure_wallet(node: &Rpc, name: &str) -> Result<()> {
    // load_on_startup: bitcoind reloads it by itself after a restart
    let load = json!({ "filename": name, "load_on_startup": true });
    match node.call::<serde_json::Value>("loadwallet", load).await {
        Ok(_) => Ok(()),
        Err(e) if e.code() == Some(RPC_WALLET_ALREADY_LOADED) => Ok(()),
        Err(e) if e.code() == Some(RPC_WALLET_NOT_FOUND) => {
            info!(name, "creating faucet wallet");
            let create = json!({ "wallet_name": name, "load_on_startup": true });
            node.call::<serde_json::Value>("createwallet", create).await.context("createwallet")?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
