//! Pool coordinator: tables people join before sharing one UTXO.
//!
//! The server only seats people and funds the pool (from the faucet, v1). Every
//! browser computes the same tree from the table and builds its own exit.

use std::{str::FromStr, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
};
use bitcoin::{Address, Amount, XOnlyPublicKey};
use ctv_core::contracts::pool::{MAX_MEMBERS, Pool, PoolMember, PoolParams};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    api::{ApiError, ApiResult, AppState, regtest_address},
    events::Event,
    registry::{PoolMemberJson, PoolState, PoolTable},
};

/// Each member's share of the pool, paid by the faucet.
const SHARE_SAT: u64 = 100_000;
/// Paid by whoever leaves.
const FEE_SAT: u64 = 500;

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError::bad_request(msg)
}

fn announce(state: &AppState, table: &PoolTable, change: &str) {
    let _ = state.chain.events.send(Event::PoolUpdated {
        id: table.id.clone(),
        name: table.name.clone(),
        members: table.members.iter().map(|m| m.label.clone()).collect(),
        state: table.state,
        change: change.into(),
        amount_sat: table.share_sat * table.members.len() as u64,
    });
}

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Vec<PoolTable>> {
    Json(state.registry.pools().await)
}

#[derive(Deserialize)]
pub struct CreateReq {
    name: String,
    host: String,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateReq>,
) -> ApiResult<PoolTable> {
    let name = req.name.trim();
    let host = req.host.trim();
    if name.is_empty() || name.chars().count() > 24 || host.is_empty() || host.chars().count() > 24
    {
        return Err(bad("name and host must be 1..=24 characters"));
    }
    let table = state.registry.create_pool(name.into(), host.into(), SHARE_SAT, FEE_SAT).await;
    announce(&state, &table, "created");
    Ok(Json(table))
}

pub async fn join(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(member): Json<PoolMemberJson>,
) -> ApiResult<PoolTable> {
    let label = member.label.trim().to_owned();
    if label.is_empty() || label.chars().count() > 24 {
        return Err(bad("label must be 1..=24 characters"));
    }
    XOnlyPublicKey::from_str(&member.pubkey).map_err(|_| bad("invalid public key"))?;
    let address = regtest_address(&member.address)?;
    let member = PoolMemberJson { label, pubkey: member.pubkey, address };

    let (joined, table) = state
        .registry
        .update_pool(&id, |t| {
            if t.members.iter().any(|m| m.pubkey == member.pubkey) {
                return Ok(false); // already seated: idempotent
            }
            if t.state != PoolState::Open {
                return Err("this table has already started".into());
            }
            if t.members.len() >= MAX_MEMBERS {
                return Err(format!("tables have at most {MAX_MEMBERS} seats"));
            }
            t.members.push(member);
            Ok(true)
        })
        .await
        .map_err(bad)?;
    if joined {
        announce(&state, &table, "joined");
    }
    Ok(Json(table))
}

pub async fn start(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<PoolTable> {
    // Claim the start atomically so two clicks can't fund it twice.
    let (_, table) = state
        .registry
        .update_pool(&id, |t| {
            if t.state != PoolState::Open {
                return Err("this table has already started".into());
            }
            if t.members.len() < 2 {
                return Err("a pool needs at least 2 people".into());
            }
            t.state = PoolState::Funding;
            Ok(())
        })
        .await
        .map_err(bad)?;

    match fund(&state, &table).await {
        Ok(outpoint) => {
            let (_, table) = state
                .registry
                .update_pool(&id, |t| {
                    t.state = PoolState::Funded;
                    t.outpoint = Some(outpoint);
                    Ok(())
                })
                .await
                .map_err(bad)?;
            announce(&state, &table, "funded");
            Ok(Json(table))
        }
        Err(e) => {
            // Back to open so they can try again.
            let _ = state
                .registry
                .update_pool(&id, |t| {
                    t.state = PoolState::Open;
                    Ok(())
                })
                .await;
            Err(e)
        }
    }
}

/// Sends `members × share` to the pool address and returns the funded outpoint.
async fn fund(state: &AppState, table: &PoolTable) -> Result<String, ApiError> {
    let pool = build(table)?;
    let address = pool.address(bitcoin::Network::Regtest);
    let txid = state.chain.send(&address.to_string(), pool.amount().to_sat()).await?;
    let tx: Value = state.chain.node.call("getrawtransaction", json!([txid, true])).await?;
    let spk = address.script_pubkey().to_hex_string();
    let vout = tx["vout"]
        .as_array()
        .and_then(|outs| outs.iter().position(|o| o["scriptPubKey"]["hex"] == spk.as_str()))
        .ok_or_else(|| anyhow::anyhow!("funding transaction doesn't pay the pool"))?;
    if state.chain.miner.borrow().paused {
        state.chain.mine(1).await?;
    }
    Ok(format!("{txid}:{vout}"))
}

fn build(table: &PoolTable) -> Result<Pool, ApiError> {
    let members = table
        .members
        .iter()
        .map(|m| {
            Ok(PoolMember {
                label: m.label.clone(),
                pubkey: XOnlyPublicKey::from_str(&m.pubkey)
                    .map_err(|_| bad("invalid public key"))?,
                payout: Address::from_str(&m.address)
                    .map_err(|_| bad("invalid address"))?
                    .assume_checked()
                    .script_pubkey(),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    Pool::new(PoolParams {
        members,
        share: Amount::from_sat(table.share_sat),
        fee: Amount::from_sat(table.fee_sat),
    })
    .map_err(|e| bad(e.to_string()))
}
