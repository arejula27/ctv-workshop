use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result, bail};
use bitcoin::Amount;

#[derive(Clone, Debug)]
pub struct Config {
    pub listen: SocketAddr,
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_pass: String,
    pub wallet: String,
    pub admin_token: String,
    pub faucet_amount: Amount,
    pub faucet_cooldown: Duration,
    pub miner_interval: Duration,
    pub miner_paused: bool,
    /// Blocks mined at startup so the faucet has mature coinbases.
    pub seed_blocks: u64,
    pub max_jump: u64,
    /// Where the room registry is saved; in memory only when unset.
    pub registry_path: Option<std::path::PathBuf>,
}

pub const MIN_MINER_INTERVAL: Duration = Duration::from_secs(2);

impl Config {
    pub fn from_env() -> Result<Self> {
        let admin_token = required("ADMIN_TOKEN")?;
        if admin_token.len() < 8 {
            bail!("ADMIN_TOKEN must be at least 8 characters");
        }
        let miner_interval = Duration::from_secs(parse("MINER_INTERVAL_S", 30)?);
        if miner_interval < MIN_MINER_INTERVAL {
            bail!("MINER_INTERVAL_S must be >= {}", MIN_MINER_INTERVAL.as_secs());
        }
        Ok(Self {
            listen: parse("LISTEN_ADDR", "0.0.0.0:3000".parse()?)?,
            rpc_url: var("BITCOIND_RPC_URL").unwrap_or_else(|| "http://127.0.0.1:18443".into()),
            rpc_user: required("RPC_USER")?,
            rpc_pass: required("RPC_PASS")?,
            wallet: var("FAUCET_WALLET").unwrap_or_else(|| "faucet".into()),
            admin_token,
            faucet_amount: Amount::from_sat(parse("FAUCET_AMOUNT_SAT", 1_000_000)?),
            faucet_cooldown: Duration::from_secs(parse("FAUCET_COOLDOWN_S", 60)?),
            miner_interval,
            miner_paused: parse("MINER_PAUSED", false)?,
            seed_blocks: parse("SEED_BLOCKS", 200)?,
            max_jump: parse("MAX_JUMP", 1000)?,
            registry_path: var("REGISTRY_PATH").map(Into::into),
        })
    }
}

fn var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

fn required(key: &str) -> Result<String> {
    var(key).with_context(|| format!("missing env var {key}"))
}

fn parse<T: std::str::FromStr>(key: &str, default: T) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    match var(key) {
        None => Ok(default),
        Some(v) => v.parse().map_err(|e| anyhow::anyhow!("invalid {key}={v}: {e}")),
    }
}
