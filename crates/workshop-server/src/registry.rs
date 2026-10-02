//! Who is in the room and which vaults exist. Lets a thief find a vault from a
//! stolen hot key alone, and lets `/screen` show every vault.
//!
//! Persisted as JSON so a server restart doesn't forget the room; `reset.sh`
//! wipes it together with the chain.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::warn;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Participant {
    pub name: String,
    pub address: String,
    pub joined_at: u64,
}

/// Mirror of the webapp's `VaultParams` (and ctv_core's `VaultParams`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VaultParams {
    pub amount_sat: u64,
    pub hot_pubkey: String,
    pub cold_pubkey: String,
    pub delay: u16,
    pub fee_sat: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VaultEntry {
    pub owner: String,
    pub params: VaultParams,
    pub outpoint: String,
    pub registered_at: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContractKind {
    Inheritance,
    Subscription,
}

/// An inheritance or subscription. `params` is the webapp's JSON, validated by
/// `room` before it gets here. `id` is stable while `outpoint` can move (an
/// inheritance's "I'm alive" re-creates it).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractEntry {
    pub id: String,
    pub kind: ContractKind,
    pub owner: String,
    pub params: serde_json::Value,
    pub outpoint: String,
    pub registered_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PoolState {
    /// People can join.
    Open,
    /// The server is funding it; guards against a double start.
    Funding,
    Funded,
    /// Everyone has left.
    Dissolved,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PoolMemberJson {
    pub label: String,
    pub pubkey: String,
    /// The member's on-chain account: exits and change go here.
    pub address: String,
    /// Current balance inside the pool.
    #[serde(default)]
    pub balance_sat: u64,
}

/// A coin a member brings into a deposit.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UtxoJson {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub address: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PoolOp {
    /// Pay another member inside the pool.
    Pay,
    /// Send from the pool to any on-chain address.
    Withdraw,
    /// Bring coins from your on-chain account into the pool.
    Deposit,
}

/// An update waiting for everyone's signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolProposal {
    pub op: PoolOp,
    pub by: String,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    pub amount_sat: u64,
    /// The members and balances after the update.
    pub next: Vec<PoolMemberJson>,
    #[serde(default)]
    pub deposit_utxos: Vec<UtxoJson>,
    /// Outputs after the pool (withdrawal payout, deposit change).
    #[serde(default)]
    pub outputs: Vec<(String, u64)>,
    pub txid: String,
    /// Signatures so far, by pubkey.
    #[serde(default)]
    pub sigs: BTreeMap<String, String>,
    /// Key-path witnesses of the deposit inputs, by input index.
    #[serde(default)]
    pub input_witnesses: BTreeMap<usize, Vec<String>>,
    pub created_at: u64,
}

/// One step of the pool's history, for the app and the explorer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolStep {
    /// "funded", "pay", "withdraw", "deposit" or "exit".
    pub kind: String,
    pub by: String,
    #[serde(default)]
    pub to: Option<String>,
    pub amount_sat: u64,
    pub txid: String,
    pub at: u64,
}

/// A pool table: who sits at it and, once funded, where the shared UTXO is.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolTable {
    pub id: String,
    pub name: String,
    pub host: String,
    pub members: Vec<PoolMemberJson>,
    pub share_sat: u64,
    pub fee_sat: u64,
    pub state: PoolState,
    pub outpoint: Option<String>,
    pub created_at: u64,
    #[serde(default)]
    pub pending: Option<PoolProposal>,
    #[serde(default)]
    pub history: Vec<PoolStep>,
}

/// The demo the whole room is playing right now (they run one after another).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Demo {
    #[default]
    Vault,
    Inheritance,
    Subscription,
    Pool,
}

/// A participant's hot wallet in the vault game. Its seed is public on purpose:
/// it's what the thieves get to play with.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotWallet {
    pub owner: String,
    pub address: String,
    pub pubkey: String,
    pub seed: String,
    pub created_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Data {
    /// Keyed by the participant's main address.
    participants: BTreeMap<String, Participant>,
    /// Keyed by funding outpoint.
    vaults: BTreeMap<String, VaultEntry>,
    /// Keyed by id. `default` keeps registries saved before F6 loading.
    #[serde(default)]
    contracts: BTreeMap<String, ContractEntry>,
    #[serde(default)]
    pools: BTreeMap<String, PoolTable>,
    #[serde(default)]
    demo: Demo,
    /// Keyed by address.
    #[serde(default)]
    hot_wallets: BTreeMap<String, HotWallet>,
}

pub struct Registry {
    path: Option<PathBuf>,
    data: RwLock<Data>,
}

impl Registry {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let data = match &path {
            Some(p) if p.exists() => {
                let raw = std::fs::read(p).with_context(|| format!("reading {}", p.display()))?;
                serde_json::from_slice(&raw).with_context(|| format!("parsing {}", p.display()))?
            }
            _ => Data::default(),
        };
        Ok(Self { path, data: RwLock::new(data) })
    }

    /// Returns true when the participant is new.
    pub async fn join(&self, name: String, address: String) -> bool {
        let mut data = self.data.write().await;
        let is_new = !data.participants.contains_key(&address);
        let joined_at = data.participants.get(&address).map_or_else(now, |p| p.joined_at);
        data.participants.insert(address.clone(), Participant { name, address, joined_at });
        self.persist(&data);
        is_new
    }

    pub async fn participants(&self) -> Vec<Participant> {
        let mut list: Vec<_> = self.data.read().await.participants.values().cloned().collect();
        list.sort_by_key(|p| p.joined_at);
        list
    }

    /// Returns true when the vault is new.
    pub async fn add_vault(&self, owner: String, params: VaultParams, outpoint: String) -> bool {
        let mut data = self.data.write().await;
        if data.vaults.contains_key(&outpoint) {
            return false;
        }
        let entry = VaultEntry { owner, params, outpoint: outpoint.clone(), registered_at: now() };
        data.vaults.insert(outpoint, entry);
        self.persist(&data);
        true
    }

    pub async fn vaults(&self) -> Vec<VaultEntry> {
        let mut list: Vec<_> = self.data.read().await.vaults.values().cloned().collect();
        list.sort_by_key(|v| v.registered_at);
        list
    }

    /// Inserts or moves a contract. Returns `Some(is_new)`, or `None` when nothing changed.
    pub async fn upsert_contract(
        &self,
        id: String,
        kind: ContractKind,
        owner: String,
        params: serde_json::Value,
        outpoint: String,
    ) -> Option<bool> {
        let mut data = self.data.write().await;
        let existing = data.contracts.get(&id);
        if existing.is_some_and(|c| c.outpoint == outpoint) {
            return None;
        }
        let is_new = existing.is_none();
        let registered_at = existing.map_or_else(now, |c| c.registered_at);
        let entry = ContractEntry {
            id: id.clone(),
            kind,
            owner,
            params,
            outpoint,
            registered_at,
            updated_at: now(),
        };
        data.contracts.insert(id, entry);
        self.persist(&data);
        Some(is_new)
    }

    pub async fn contracts(&self) -> Vec<ContractEntry> {
        let mut list: Vec<_> = self.data.read().await.contracts.values().cloned().collect();
        list.sort_by_key(|c| c.registered_at);
        list
    }

    /// Returns true when the wallet is new.
    pub async fn add_hot_wallet(&self, w: HotWallet) -> bool {
        let mut data = self.data.write().await;
        if data.hot_wallets.contains_key(&w.address) {
            return false;
        }
        data.hot_wallets.insert(w.address.clone(), w);
        self.persist(&data);
        true
    }

    pub async fn hot_wallets(&self) -> Vec<HotWallet> {
        let mut list: Vec<_> = self.data.read().await.hot_wallets.values().cloned().collect();
        list.sort_by_key(|w| w.created_at);
        list
    }

    pub async fn demo(&self) -> Demo {
        self.data.read().await.demo
    }

    pub async fn set_demo(&self, demo: Demo) {
        let mut data = self.data.write().await;
        data.demo = demo;
        self.persist(&data);
    }

    pub async fn pools(&self) -> Vec<PoolTable> {
        let mut list: Vec<_> = self.data.read().await.pools.values().cloned().collect();
        list.sort_by_key(|t| t.created_at);
        list
    }

    pub async fn pool(&self, id: &str) -> Option<PoolTable> {
        self.data.read().await.pools.get(id).cloned()
    }

    pub async fn create_pool(
        &self,
        name: String,
        host: String,
        share_sat: u64,
        fee_sat: u64,
    ) -> PoolTable {
        let mut data = self.data.write().await;
        // short, readable id; collisions are retried
        let mut n = now();
        let id = loop {
            let id = format!("{:05x}", n % 0xfffff);
            if !data.pools.contains_key(&id) {
                break id;
            }
            n += 1;
        };
        let table = PoolTable {
            id: id.clone(),
            name,
            host,
            members: vec![],
            share_sat,
            fee_sat,
            state: PoolState::Open,
            outpoint: None,
            created_at: now(),
            pending: None,
            history: vec![],
        };
        data.pools.insert(id, table.clone());
        self.persist(&data);
        table
    }

    /// Applies `f` to the table atomically and saves it.
    pub async fn update_pool<T>(
        &self,
        id: &str,
        f: impl FnOnce(&mut PoolTable) -> Result<T, String>,
    ) -> Result<(T, PoolTable), String> {
        let mut data = self.data.write().await;
        let table = data.pools.get_mut(id).ok_or("no such table")?;
        let out = f(table)?;
        let table = table.clone();
        self.persist(&data);
        Ok((out, table))
    }

    fn persist(&self, data: &Data) {
        let Some(path) = &self.path else { return };
        // write + rename so a crash never leaves a half-written file
        let tmp = path.with_extension("tmp");
        let res = serde_json::to_vec_pretty(data)
            .map_err(anyhow::Error::from)
            .and_then(|json| Ok(std::fs::write(&tmp, json)?))
            .and_then(|_| Ok(std::fs::rename(&tmp, path)?));
        if let Err(e) = res {
            warn!("could not persist registry to {}: {e:#}", path.display());
        }
    }
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> VaultParams {
        VaultParams {
            amount_sat: 1,
            hot_pubkey: "h".into(),
            cold_pubkey: "c".into(),
            delay: 1,
            fee_sat: 1,
        }
    }

    #[tokio::test]
    async fn persists_and_reloads() {
        let path = std::env::temp_dir().join(format!("registry-test-{}.json", std::process::id()));
        let reg = Registry::load(Some(path.clone())).unwrap();
        assert!(reg.join("Ana".into(), "addr1".into()).await);
        assert!(!reg.join("Ana María".into(), "addr1".into()).await, "same address updates");
        assert!(reg.add_vault("Ana".into(), params(), "tx:0".into()).await);
        assert!(!reg.add_vault("Ana".into(), params(), "tx:0".into()).await);

        let again = Registry::load(Some(path.clone())).unwrap();
        assert_eq!(again.participants().await[0].name, "Ana María");
        assert_eq!(again.vaults().await.len(), 1);
        std::fs::remove_file(path).unwrap();
    }
}
