//! The room: participants, registered contracts and what people do with them.
//!
//! Nothing here is trusted blindly: contracts are recomputed with `ctv-core` and
//! checked against the chain, and activities must point at a real transaction.

use std::{
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{Json, extract::State};
use bitcoin::{Address, Amount, OutPoint, ScriptBuf, XOnlyPublicKey, hex::FromHex};
use ctv_core::contracts::{
    inheritance::{Heir, Inheritance, InheritanceParams},
    subscription::{Subscription, SubscriptionParams},
    vault::{Vault, VaultParams as CoreParams},
};
use ctv_core::wallet::{Role, Wallet};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    api::{ApiError, ApiResult, AppState, regtest_address},
    events::{ActivityKind, Event},
    registry::{ContractEntry, ContractKind, HotWallet, Participant, VaultEntry, VaultParams},
};

const MAX_NAME: usize = 24;

fn clean_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(ApiError::bad_request(format!("name must be 1..={MAX_NAME} characters")));
    }
    Ok(name.to_owned())
}

// ---------- participants ----------

#[derive(Deserialize)]
pub struct JoinReq {
    name: String,
    address: String,
}

pub async fn join(
    State(state): State<Arc<AppState>>,
    Json(req): Json<JoinReq>,
) -> ApiResult<Value> {
    let name = clean_name(&req.name)?;
    let address = regtest_address(&req.address)?;
    if state.registry.join(name.clone(), address).await {
        let _ = state.chain.events.send(Event::ParticipantJoined { name });
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Serialize)]
pub struct PublicParticipant {
    name: String,
    /// Main address, so others can name them as heir or subscription receiver.
    address: String,
}

pub async fn participants(State(state): State<Arc<AppState>>) -> Json<Vec<PublicParticipant>> {
    let list = state.registry.participants().await;
    Json(
        list.into_iter()
            .map(|Participant { name, address, .. }| PublicParticipant { name, address })
            .collect(),
    )
}

// ---------- vaults ----------

#[derive(Deserialize)]
pub struct VaultReq {
    owner: String,
    params: VaultParams,
    outpoint: String,
}

pub async fn register_vault(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VaultReq>,
) -> ApiResult<Value> {
    let owner = clean_name(&req.owner)?;
    let outpoint =
        OutPoint::from_str(&req.outpoint).map_err(|_| ApiError::bad_request("invalid outpoint"))?;
    let vault = rebuild(&req.params)?;

    check_pays_to(&state, outpoint, &vault.contract.script_pubkey()).await?;

    if state.registry.add_vault(owner.clone(), req.params.clone(), req.outpoint.clone()).await {
        let _ = state.chain.events.send(Event::VaultRegistered {
            owner,
            amount_sat: req.params.amount_sat,
            delay: req.params.delay,
            outpoint: req.outpoint,
        });
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn vaults(State(state): State<Arc<AppState>>) -> Json<Vec<VaultEntry>> {
    Json(state.registry.vaults().await)
}

fn rebuild(p: &VaultParams) -> Result<Vault, ApiError> {
    let key = |s: &str| {
        XOnlyPublicKey::from_str(s).map_err(|_| ApiError::bad_request("invalid public key"))
    };
    Vault::new(CoreParams {
        amount: Amount::from_sat(p.amount_sat),
        hot: key(&p.hot_pubkey)?,
        cold: key(&p.cold_pubkey)?,
        delay: p.delay,
        fee: Amount::from_sat(p.fee_sat),
    })
    .map_err(|e| ApiError::bad_request(e.to_string()))
}

// ---------- activity ----------

#[derive(Deserialize)]
pub struct ActivityReq {
    who: String,
    kind: ActivityKind,
    /// The vault's funding outpoint, a contract or pool id, or a hot wallet address.
    outpoint: String,
    txid: String,
    /// Only for wallet sweeps, where there's no registered amount.
    #[serde(default)]
    amount_sat: Option<u64>,
}

pub async fn activity(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ActivityReq>,
) -> ApiResult<Value> {
    let who = clean_name(&req.who)?;
    // Vaults by funding outpoint; other contracts by id or current outpoint.
    let vault = state.registry.vaults().await.into_iter().find(|v| v.outpoint == req.outpoint);
    let (owner, amount_sat, outpoint) = match vault {
        Some(v) => (v.owner, v.params.amount_sat, v.outpoint),
        None if let Some(t) = state.registry.pool(&req.outpoint).await => {
            (t.host, t.share_sat, t.id)
        }
        None if let Some(w) =
            state.registry.hot_wallets().await.into_iter().find(|w| w.address == req.outpoint) =>
        {
            (w.owner, req.amount_sat.unwrap_or_default(), w.address)
        }
        None => {
            let c = state
                .registry
                .contracts()
                .await
                .into_iter()
                .find(|c| c.id == req.outpoint || c.outpoint == req.outpoint)
                .ok_or_else(|| ApiError::bad_request("unknown contract"))?;
            let amount = headline_amount(&c);
            (c.owner, amount, c.id)
        }
    };
    // Must be a transaction the node has actually seen.
    get_tx(&state, &req.txid).await?;
    // One event per transaction: clicking twice (or rebroadcasting) mustn't repeat it.
    if !state.seen_activity.lock().await.insert(req.txid.clone()) {
        return Ok(Json(json!({ "ok": true, "duplicate": true })));
    }
    let _ = state.chain.events.send(Event::Activity {
        who,
        kind: req.kind,
        owner,
        amount_sat,
        outpoint,
        txid: req.txid,
    });
    Ok(Json(json!({ "ok": true })))
}

// ---------- hot wallets (vault game) ----------

#[derive(Deserialize)]
pub struct HotWalletReq {
    owner: String,
    seed: String,
}

/// Publishes a hot wallet with its seed, so thieves can try their luck.
pub async fn register_hot_wallet(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HotWalletReq>,
) -> ApiResult<Value> {
    let owner = clean_name(&req.owner)?;
    let seed = <[u8; 32]>::from_hex(req.seed.trim())
        .map_err(|_| ApiError::bad_request("seed must be 32 bytes of hex"))?;
    // Derive everything from the seed so nobody can publish a mismatched wallet.
    let w = Wallet::from_seed(seed);
    let address = w.address(Role::Main, bitcoin::Network::Regtest).to_string();
    let pubkey = w.keypair(Role::Main).x_only_public_key().0.to_string();
    let entry = HotWallet {
        owner: owner.clone(),
        address: address.clone(),
        pubkey,
        seed: req.seed.trim().to_lowercase(),
        created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
            as u64,
    };
    if state.registry.add_hot_wallet(entry).await {
        let _ = state.chain.events.send(Event::HotWalletCreated { owner });
    }
    Ok(Json(json!({ "address": address })))
}

pub async fn hot_wallets(State(state): State<Arc<AppState>>) -> Json<Vec<HotWallet>> {
    Json(state.registry.hot_wallets().await)
}

// ---------- inheritances and subscriptions ----------

/// Mirror of the webapp's params (crates/ctv-wasm/src/lib.rs).
#[derive(Deserialize)]
struct InheritanceJson {
    amount_sat: u64,
    owner_pubkey: String,
    heirs: Vec<HeirJson>,
    delay: u16,
    fee_sat: u64,
}

#[derive(Deserialize)]
struct HeirJson {
    label: String,
    address: String,
    share: u32,
}

#[derive(Deserialize)]
struct SubscriptionJson {
    payment_sat: u64,
    count: u16,
    every: u16,
    receiver_address: String,
    payer_pubkey: String,
    fee_sat: u64,
}

#[derive(Deserialize)]
pub struct ContractReq {
    id: String,
    kind: ContractKind,
    owner: String,
    params: Value,
    outpoint: String,
}

pub async fn register_contract(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ContractReq>,
) -> ApiResult<Value> {
    let owner = clean_name(&req.owner)?;
    if req.id.is_empty() || req.id.len() > 100 {
        return Err(ApiError::bad_request("invalid id"));
    }
    let outpoint =
        OutPoint::from_str(&req.outpoint).map_err(|_| ApiError::bad_request("invalid outpoint"))?;
    let bad = |e: String| ApiError::bad_request(e);
    let (spk, amount_sat) = match req.kind {
        ContractKind::Inheritance => {
            let p: InheritanceJson =
                serde_json::from_value(req.params.clone()).map_err(|e| bad(e.to_string()))?;
            let heirs = p
                .heirs
                .iter()
                .map(|h| {
                    Ok(Heir {
                        label: h.label.clone(),
                        script_pubkey: script(&h.address)?,
                        share: h.share,
                    })
                })
                .collect::<Result<_, ApiError>>()?;
            let i = Inheritance::new(InheritanceParams {
                amount: Amount::from_sat(p.amount_sat),
                owner: xonly(&p.owner_pubkey)?,
                heirs,
                delay: p.delay,
                fee: Amount::from_sat(p.fee_sat),
            })
            .map_err(|e| bad(e.to_string()))?;
            (i.contract.script_pubkey(), p.amount_sat)
        }
        ContractKind::Subscription => {
            let p: SubscriptionJson =
                serde_json::from_value(req.params.clone()).map_err(|e| bad(e.to_string()))?;
            let s = Subscription::new(SubscriptionParams {
                payment: Amount::from_sat(p.payment_sat),
                count: p.count,
                every: p.every,
                receiver: script(&p.receiver_address)?,
                payer: xonly(&p.payer_pubkey)?,
                fee: Amount::from_sat(p.fee_sat),
            })
            .map_err(|e| bad(e.to_string()))?;
            (s.contract.script_pubkey(), s.amount().to_sat())
        }
    };
    check_pays_to(&state, outpoint, &spk).await?;

    if let Some(is_new) = state
        .registry
        .upsert_contract(req.id.clone(), req.kind, owner.clone(), req.params, req.outpoint)
        .await
    {
        let _ = state.chain.events.send(Event::ContractRegistered {
            kind: req.kind,
            owner,
            amount_sat,
            id: req.id,
            renewed: !is_new,
        });
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn contracts(State(state): State<Arc<AppState>>) -> Json<Vec<ContractEntry>> {
    Json(state.registry.contracts().await)
}

/// The amount a feed line talks about: the pot, or one subscription payment.
fn headline_amount(c: &ContractEntry) -> u64 {
    let field = match c.kind {
        ContractKind::Inheritance => "amount_sat",
        ContractKind::Subscription => "payment_sat",
    };
    c.params[field].as_u64().unwrap_or_default()
}

fn xonly(s: &str) -> Result<XOnlyPublicKey, ApiError> {
    XOnlyPublicKey::from_str(s).map_err(|_| ApiError::bad_request("invalid public key"))
}

fn script(address: &str) -> Result<ScriptBuf, ApiError> {
    let checked = regtest_address(address)?;
    Ok(Address::from_str(&checked)
        .expect("validated by regtest_address")
        .assume_checked()
        .script_pubkey())
}

/// The outpoint must really pay to `spk`.
async fn check_pays_to(
    state: &AppState,
    outpoint: OutPoint,
    spk: &ScriptBuf,
) -> Result<(), ApiError> {
    let tx = get_tx(state, &outpoint.txid.to_string()).await?;
    let got =
        tx["vout"][outpoint.vout as usize]["scriptPubKey"]["hex"].as_str().unwrap_or_default();
    if got != spk.to_hex_string() {
        return Err(ApiError::bad_request("that output does not pay to this contract"));
    }
    Ok(())
}

async fn get_tx(state: &AppState, txid: &str) -> Result<Value, ApiError> {
    state
        .chain
        .node
        .call::<Value>("getrawtransaction", json!([txid, true]))
        .await
        .map_err(|_| ApiError::bad_request("transaction not found"))
}
