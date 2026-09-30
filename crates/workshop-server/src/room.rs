//! The room: participants, registered vaults and what people do with them.
//!
//! Nothing here is trusted blindly: vaults are recomputed with `ctv-core` and
//! checked against the chain, and activities must point at a real transaction.

use std::{str::FromStr, sync::Arc};

use axum::{Json, extract::State};
use bitcoin::{Amount, OutPoint, XOnlyPublicKey};
use ctv_core::contracts::vault::{Vault, VaultParams as CoreParams};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    api::{ApiError, ApiResult, AppState, regtest_address},
    events::{ActivityKind, Event},
    registry::{Participant, VaultEntry, VaultParams},
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
}

pub async fn participants(State(state): State<Arc<AppState>>) -> Json<Vec<PublicParticipant>> {
    let list = state.registry.participants().await;
    Json(list.into_iter().map(|Participant { name, .. }| PublicParticipant { name }).collect())
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

    // The outpoint must really pay to this vault.
    let tx = get_tx(&state, &outpoint.txid.to_string()).await?;
    let spk =
        tx["vout"][outpoint.vout as usize]["scriptPubKey"]["hex"].as_str().unwrap_or_default();
    if spk != vault.contract.script_pubkey().to_hex_string() {
        return Err(ApiError::bad_request("that output does not pay to this vault"));
    }

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
    /// The vault's funding outpoint.
    outpoint: String,
    txid: String,
}

pub async fn activity(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ActivityReq>,
) -> ApiResult<Value> {
    let who = clean_name(&req.who)?;
    let vault = state
        .registry
        .vaults()
        .await
        .into_iter()
        .find(|v| v.outpoint == req.outpoint)
        .ok_or_else(|| ApiError::bad_request("unknown vault"))?;
    // Must be a transaction the node has actually seen.
    get_tx(&state, &req.txid).await?;
    let _ = state.chain.events.send(Event::Activity {
        who,
        kind: req.kind,
        owner: vault.owner,
        amount_sat: vault.params.amount_sat,
        outpoint: vault.outpoint,
        txid: req.txid,
    });
    Ok(Json(json!({ "ok": true })))
}

async fn get_tx(state: &AppState, txid: &str) -> Result<Value, ApiError> {
    state
        .chain
        .node
        .call::<Value>("getrawtransaction", json!([txid, true]))
        .await
        .map_err(|_| ApiError::bad_request("transaction not found"))
}
