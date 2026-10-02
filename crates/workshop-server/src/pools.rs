//! Pool coordinator: tables people join before sharing one UTXO, and the
//! cooperative updates (pay, withdraw, deposit) that need everyone's signature.
//!
//! The server never holds keys. It proposes an update; every app rebuilds the
//! same transaction from the proposal, checks it doesn't take its money, and
//! signs. With every signature in, the server assembles and broadcasts it.
//! Exits don't go through here at all (CTV), the app just reports them.

use std::{str::FromStr, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
};
use bitcoin::{
    Address, Amount, OutPoint, ScriptBuf, Transaction, TxOut, Txid, Witness, XOnlyPublicKey,
    consensus::encode::serialize_hex, hex::FromHex, secp256k1::schnorr::Signature,
};
use ctv_core::{
    contracts::pool::{MAX_MEMBERS, Pool, PoolMember, PoolParams},
    wallet::{DUST, Utxo},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    api::{ApiError, ApiResult, AppState, regtest_address},
    events::Event,
    registry::{
        PoolMemberJson, PoolOp, PoolProposal, PoolState, PoolStep, PoolTable, UtxoJson, now,
    },
};

/// Each member's starting balance, paid by the house when the table starts.
const SHARE_SAT: u64 = 100_000;
/// Paid by whoever leaves, and by whoever starts an update.
const FEE_SAT: u64 = 500;
/// A proposal nobody finished signing is dropped after this long.
const PROPOSAL_TTL_MS: u64 = 120_000;

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError::bad_request(msg)
}

fn announce(
    state: &AppState,
    t: &PoolTable,
    change: &str,
    by: Option<&str>,
    to: Option<&str>,
    amount: u64,
) {
    let _ = state.chain.events.send(Event::PoolUpdated {
        id: t.id.clone(),
        name: t.name.clone(),
        members: t.members.iter().map(|m| m.label.clone()).collect(),
        state: t.state,
        change: change.into(),
        amount_sat: amount,
        by: by.map(Into::into),
        to: to.map(Into::into),
    });
}

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Vec<PoolTable>> {
    Json(state.registry.pools().await)
}

// ---------- tables ----------

#[derive(Deserialize)]
pub struct CreateReq {
    name: String,
    host: String,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateReq>,
) -> ApiResult<PoolTable> {
    let (name, host) = (req.name.trim(), req.host.trim());
    if name.is_empty() || name.chars().count() > 24 || host.is_empty() || host.chars().count() > 24
    {
        return Err(bad("name and host must be 1..=24 characters"));
    }
    let table = state.registry.create_pool(name.into(), host.into(), SHARE_SAT, FEE_SAT).await;
    announce(&state, &table, "created", Some(host), None, 0);
    Ok(Json(table))
}

#[derive(Deserialize)]
pub struct JoinReq {
    label: String,
    pubkey: String,
    address: String,
}

pub async fn join(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<JoinReq>,
) -> ApiResult<PoolTable> {
    let label = req.label.trim().to_owned();
    if label.is_empty() || label.chars().count() > 24 {
        return Err(bad("label must be 1..=24 characters"));
    }
    XOnlyPublicKey::from_str(&req.pubkey).map_err(|_| bad("invalid public key"))?;
    let member = PoolMemberJson {
        label,
        pubkey: req.pubkey,
        address: regtest_address(&req.address)?,
        balance_sat: 0,
    };

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
            t.members.push(member.clone());
            Ok(true)
        })
        .await
        .map_err(bad)?;
    if joined {
        announce(&state, &table, "joined", Some(&member.label), None, 0);
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
            for m in &mut t.members {
                m.balance_sat = t.share_sat;
            }
            t.state = PoolState::Funding;
            Ok(())
        })
        .await
        .map_err(bad)?;

    match fund(&state, &table).await {
        Ok(outpoint) => {
            let txid = outpoint.split(':').next().unwrap_or_default().to_owned();
            let total = table.share_sat * table.members.len() as u64;
            let (_, table) = state
                .registry
                .update_pool(&id, |t| {
                    t.state = PoolState::Funded;
                    t.outpoint = Some(outpoint);
                    t.history.push(PoolStep {
                        kind: "funded".into(),
                        by: t.host.clone(),
                        to: None,
                        amount_sat: total,
                        txid,
                        at: now(),
                    });
                    Ok(())
                })
                .await
                .map_err(bad)?;
            announce(&state, &table, "funded", None, None, total);
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

/// Sends the starting balances to the pool address; returns the funded outpoint.
async fn fund(state: &AppState, table: &PoolTable) -> Result<String, ApiError> {
    let pool = build(&table.members, table.fee_sat)?;
    let address = pool.address(bitcoin::Network::Regtest);
    let txid = state.chain.send(&address.to_string(), pool.amount().to_sat()).await?;
    let vout = find_vout(state, &txid, &address.script_pubkey()).await?;
    if state.chain.miner.borrow().paused {
        state.chain.mine(1).await?;
    }
    Ok(format!("{txid}:{vout}"))
}

// ---------- cooperative updates ----------

#[derive(Deserialize)]
pub struct ProposeReq {
    /// Who starts it (their pool pubkey).
    by: String,
    op: PoolOp,
    amount_sat: u64,
    /// Pay: the member to pay (pubkey).
    to: Option<String>,
    /// Withdraw: the destination address.
    address: Option<String>,
    /// Deposit: the coins being brought in.
    #[serde(default)]
    utxos: Vec<UtxoJson>,
}

pub async fn propose(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ProposeReq>,
) -> ApiResult<PoolTable> {
    let table = state.registry.pool(&id).await.ok_or_else(|| bad("no such table"))?;
    if table.state != PoolState::Funded {
        return Err(bad("the pool isn't running"));
    }
    if table.pending.as_ref().is_some_and(|p| now() - p.created_at < PROPOSAL_TTL_MS) {
        return Err(bad("another update is waiting for signatures"));
    }
    let fee = table.fee_sat;
    let mut next = table.members.clone();
    let by = next
        .iter()
        .position(|m| m.pubkey == req.by)
        .ok_or_else(|| bad("you're not in this pool"))?;
    let by_label = next[by].label.clone();
    let mut outputs: Vec<(String, u64)> = vec![];
    let mut to_label = None;
    let debit = |m: &mut PoolMemberJson, sats: u64| -> Result<(), ApiError> {
        m.balance_sat =
            m.balance_sat.checked_sub(sats).filter(|b| *b >= fee + DUST.to_sat()).ok_or_else(
                || bad("not enough balance in the pool (to take it all, leave the pool)"),
            )?;
        Ok(())
    };
    if req.amount_sat < DUST.to_sat() {
        return Err(bad("amount too small"));
    }
    match req.op {
        PoolOp::Pay => {
            let to = next
                .iter()
                .position(|m| Some(&m.pubkey) == req.to.as_ref())
                .filter(|&i| i != by)
                .ok_or_else(|| bad("pay someone else in the pool"))?;
            debit(&mut next[by], req.amount_sat + fee)?;
            next[to].balance_sat += req.amount_sat;
            to_label = Some(next[to].label.clone());
        }
        PoolOp::Withdraw => {
            let address = regtest_address(req.address.as_deref().unwrap_or_default())?;
            debit(&mut next[by], req.amount_sat + fee)?;
            outputs.push((address, req.amount_sat));
        }
        PoolOp::Deposit => {
            let total: u64 = req.utxos.iter().map(|u| u.value).sum();
            let change = total
                .checked_sub(req.amount_sat + fee)
                .ok_or_else(|| bad("not enough on-chain funds for that deposit"))?;
            next[by].balance_sat += req.amount_sat;
            if change >= DUST.to_sat() {
                outputs.push((next[by].address.clone(), change));
            }
        }
    }
    let (_, tx) = build_update(&table, &next, &req.utxos, &outputs)?;
    let proposal = PoolProposal {
        op: req.op,
        by: by_label.clone(),
        to: to_label.clone(),
        address: req.address,
        amount_sat: req.amount_sat,
        next,
        deposit_utxos: req.utxos,
        outputs,
        txid: tx.compute_txid().to_string(),
        sigs: Default::default(),
        input_witnesses: Default::default(),
        created_at: now(),
    };
    let (_, table) = state
        .registry
        .update_pool(&id, |t| {
            t.pending = Some(proposal);
            Ok(())
        })
        .await
        .map_err(bad)?;
    announce(&state, &table, "proposal", Some(&by_label), to_label.as_deref(), req.amount_sat);
    Ok(Json(table))
}

#[derive(Deserialize)]
pub struct SignReq {
    pubkey: String,
    txid: String,
    sig: String,
    /// Deposit inputs this signer owns: (input index, witness items in hex).
    #[serde(default)]
    input_witnesses: Vec<(usize, Vec<String>)>,
}

pub async fn sign(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<SignReq>,
) -> ApiResult<PoolTable> {
    let table = state.registry.pool(&id).await.ok_or_else(|| bad("no such table"))?;
    let p = table.pending.clone().ok_or_else(|| bad("nothing to sign"))?;
    if p.txid != req.txid {
        return Err(bad("that's not the pending update"));
    }
    let (pool, tx) = build_update(&table, &p.next, &p.deposit_utxos, &p.outputs)?;
    let prevouts = pool.update_prevouts(&utxos(&p.deposit_utxos)?);
    let pubkey = XOnlyPublicKey::from_str(&req.pubkey).map_err(|_| bad("invalid public key"))?;
    let sig =
        Signature::from_slice(&Vec::from_hex(&req.sig).map_err(|_| bad("invalid signature"))?)
            .map_err(|_| bad("invalid signature"))?;
    pool.verify_update(&tx, &prevouts, &pubkey, &sig).map_err(|_| bad("bad signature"))?;

    let (_, table) = state
        .registry
        .update_pool(&id, |t| {
            let pending =
                t.pending.as_mut().filter(|q| q.txid == req.txid).ok_or("the update changed")?;
            pending.sigs.insert(req.pubkey.clone(), req.sig.clone());
            for (i, w) in &req.input_witnesses {
                pending.input_witnesses.insert(*i, w.clone());
            }
            Ok(())
        })
        .await
        .map_err(bad)?;
    let p = table.pending.clone().expect("just stored");
    let deposit_inputs = p.deposit_utxos.len();
    if p.sigs.len() < table.members.len() || p.input_witnesses.len() < deposit_inputs {
        announce(&state, &table, "signature", Some(&p.by), p.to.as_deref(), p.amount_sat);
        return Ok(Json(table));
    }

    // Everyone signed: assemble and broadcast.
    let sigs = p
        .sigs
        .iter()
        .map(|(k, s)| {
            Ok((
                XOnlyPublicKey::from_str(k).map_err(|_| bad("invalid key"))?,
                Signature::from_slice(&Vec::from_hex(s).map_err(|_| bad("invalid signature"))?)
                    .map_err(|_| bad("invalid signature"))?,
            ))
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let mut tx = pool.finalize_update(tx, &sigs).map_err(|e| bad(e.to_string()))?;
    for (i, items) in &p.input_witnesses {
        let items = items
            .iter()
            .map(|h| Vec::<u8>::from_hex(h))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| bad("bad witness"))?;
        tx.input.get_mut(*i).ok_or_else(|| bad("bad input index"))?.witness =
            Witness::from_slice(&items);
    }
    let broadcast: Result<String, _> =
        state.chain.node.call("sendrawtransaction", json!([serialize_hex(&tx)])).await;
    let kind = match p.op {
        PoolOp::Pay => "pay",
        PoolOp::Withdraw => "withdraw",
        PoolOp::Deposit => "deposit",
    };
    let (_, table) = state
        .registry
        .update_pool(&id, |t| {
            t.pending = None;
            if let Ok(txid) = &broadcast {
                t.members = p.next.clone();
                t.outpoint = Some(format!("{txid}:0"));
                t.history.push(PoolStep {
                    kind: kind.into(),
                    by: p.by.clone(),
                    to: p.to.clone(),
                    amount_sat: p.amount_sat,
                    txid: txid.clone(),
                    at: now(),
                });
            }
            Ok(())
        })
        .await
        .map_err(bad)?;
    match broadcast {
        Ok(_) => {
            announce(&state, &table, kind, Some(&p.by), p.to.as_deref(), p.amount_sat);
            Ok(Json(table))
        }
        Err(e) => {
            announce(&state, &table, "failed", Some(&p.by), None, 0);
            Err(bad(format!("the update was rejected: {e}")))
        }
    }
}

pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    let (_, table) = state
        .registry
        .update_pool(&id, |t| {
            t.pending = None;
            Ok(())
        })
        .await
        .map_err(bad)?;
    announce(&state, &table, "failed", None, None, 0);
    Ok(Json(json!({ "ok": true })))
}

// ---------- exits (CTV: no permission needed, just reported) ----------

#[derive(Deserialize)]
pub struct ExitReq {
    pubkey: String,
    txid: String,
}

/// Records that a member left. The server only checks the transaction really is
/// that member's exit from the current state and that the node has seen it.
pub async fn exited(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ExitReq>,
) -> ApiResult<PoolTable> {
    let table = state.registry.pool(&id).await.ok_or_else(|| bad("no such table"))?;
    let who = table
        .members
        .iter()
        .position(|m| m.pubkey == req.pubkey)
        .ok_or_else(|| bad("not in this pool"))?;
    let pool = build(&table.members, table.fee_sat)?;
    let outpoint = OutPoint::from_str(table.outpoint.as_deref().unwrap_or_default())
        .map_err(|_| bad("pool not funded"))?;
    let leaf = 1 + who; // after the cooperative leaf
    let expected =
        pool.contract.leaves[leaf].template.as_ref().expect("exit leaves are CTV").txid(outpoint);
    if Txid::from_str(&req.txid).ok() != Some(expected) {
        return Err(bad("that's not your exit from the current pool"));
    }
    let _: Value = state
        .chain
        .node
        .call("getrawtransaction", json!([req.txid, true]))
        .await
        .map_err(|_| bad("transaction not found"))?;

    let label = table.members[who].label.clone();
    let amount = table.members[who].balance_sat - table.fee_sat;
    let (_, table) = state
        .registry
        .update_pool(&id, |t| {
            t.members.remove(who);
            t.history.push(PoolStep {
                kind: "exit".into(),
                by: label.clone(),
                to: None,
                amount_sat: amount,
                txid: req.txid.clone(),
                at: now(),
            });
            if t.members.len() == 1 {
                // The last one gets their balance in the same transaction.
                let last = t.members.remove(0);
                t.history.push(PoolStep {
                    kind: "exit".into(),
                    by: last.label,
                    to: None,
                    amount_sat: last.balance_sat,
                    txid: req.txid.clone(),
                    at: now(),
                });
                t.state = PoolState::Dissolved;
                t.outpoint = None;
            } else {
                t.outpoint = Some(format!("{}:0", req.txid));
            }
            t.pending = None;
            Ok(())
        })
        .await
        .map_err(bad)?;
    announce(
        &state,
        &table,
        if table.state == PoolState::Dissolved { "dissolved" } else { "exit" },
        Some(&label),
        None,
        amount,
    );
    Ok(Json(table))
}

// ---------- building ----------

fn script(address: &str) -> Result<ScriptBuf, ApiError> {
    Ok(Address::from_str(address)
        .map_err(|_| bad("invalid address"))?
        .assume_checked()
        .script_pubkey())
}

fn build(members: &[PoolMemberJson], fee_sat: u64) -> Result<Pool, ApiError> {
    let members = members
        .iter()
        .map(|m| {
            Ok(PoolMember {
                label: m.label.clone(),
                pubkey: XOnlyPublicKey::from_str(&m.pubkey)
                    .map_err(|_| bad("invalid public key"))?,
                payout: script(&m.address)?,
                balance: Amount::from_sat(m.balance_sat),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    Pool::new(PoolParams { members, fee: Amount::from_sat(fee_sat) })
        .map_err(|e| bad(e.to_string()))
}

fn utxos(list: &[UtxoJson]) -> Result<Vec<Utxo>, ApiError> {
    list.iter()
        .map(|u| {
            Ok(Utxo {
                outpoint: OutPoint {
                    txid: Txid::from_str(&u.txid).map_err(|_| bad("invalid txid"))?,
                    vout: u.vout,
                },
                value: Amount::from_sat(u.value),
                script_pubkey: script(&u.address)?,
            })
        })
        .collect()
}

/// The current pool and the unsigned update into `next`. Every app builds the
/// very same transaction from the same inputs (crates/ctv-wasm `pool_update`).
fn build_update(
    table: &PoolTable,
    next: &[PoolMemberJson],
    deposit: &[UtxoJson],
    outputs: &[(String, u64)],
) -> Result<(Pool, Transaction), ApiError> {
    let now_pool = build(&table.members, table.fee_sat)?;
    let next_pool = build(next, table.fee_sat)?;
    let outpoint = OutPoint::from_str(table.outpoint.as_deref().unwrap_or_default())
        .map_err(|_| bad("pool not funded"))?;
    let extra_outputs = outputs
        .iter()
        .map(|(a, v)| Ok(TxOut { value: Amount::from_sat(*v), script_pubkey: script(a)? }))
        .collect::<Result<Vec<_>, ApiError>>()?;
    let tx = now_pool
        .update_tx(outpoint, &next_pool, &utxos(deposit)?, extra_outputs)
        .map_err(|e| bad(e.to_string()))?;
    Ok((now_pool, tx))
}

async fn find_vout(state: &AppState, txid: &str, spk: &ScriptBuf) -> Result<usize, ApiError> {
    let tx: Value = state.chain.node.call("getrawtransaction", json!([txid, true])).await?;
    let hex = spk.to_hex_string();
    tx["vout"]
        .as_array()
        .and_then(|outs| outs.iter().position(|o| o["scriptPubKey"]["hex"] == hex.as_str()))
        .ok_or_else(|| anyhow::anyhow!("funding transaction doesn't pay the pool").into())
}
