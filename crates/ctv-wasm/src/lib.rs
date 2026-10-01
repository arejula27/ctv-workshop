//! wasm-bindgen bindings over `ctv-core` for the webapp.
//!
//! Plain JSON-ish objects in and out. Seeds and secrets are hex strings; the
//! browser generates seeds with `crypto.getRandomValues`, so there's no RNG here.
//! Every function that takes a vault takes the *vault* outpoint and derives the
//! rest, which keeps the JS side dumb.

use std::str::FromStr;

use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, Transaction, Txid, XOnlyPublicKey,
    consensus::encode::serialize_hex, hex::FromHex, key::Keypair, secp256k1::SecretKey,
};
use ctv_core::{
    contracts::{
        inheritance::{Heir, Inheritance, InheritanceParams},
        subscription::{Subscription, SubscriptionParams},
        vault::{Vault, VaultParams},
    },
    taproot::SECP,
    wallet::{Role, Utxo, Wallet},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;

const NETWORK: Network = Network::Regtest;

type JsResult = Result<JsValue, JsError>;

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

// ---------- wallet ----------

#[derive(Serialize)]
struct WalletInfo {
    main_address: String,
    hot_address: String,
    cold_address: String,
    main_pubkey: String,
    hot_pubkey: String,
    cold_pubkey: String,
}

#[wasm_bindgen]
pub fn wallet_info(seed_hex: &str) -> JsResult {
    let w = wallet(seed_hex)?;
    to_js(&WalletInfo {
        main_address: w.address(Role::Main, NETWORK).to_string(),
        hot_address: w.address(Role::Hot, NETWORK).to_string(),
        cold_address: w.address(Role::Cold, NETWORK).to_string(),
        main_pubkey: w.keypair(Role::Main).x_only_public_key().0.to_string(),
        hot_pubkey: w.keypair(Role::Hot).x_only_public_key().0.to_string(),
        cold_pubkey: w.keypair(Role::Cold).x_only_public_key().0.to_string(),
    })
}

/// The hot key's secret, for the "someone stole my hot key" game.
#[wasm_bindgen]
pub fn hot_secret(seed_hex: &str) -> Result<String, JsError> {
    Ok(wallet(seed_hex)?.keypair(Role::Hot).display_secret().to_string())
}

/// Public key of a (stolen) secret, to check it matches a vault.
#[wasm_bindgen]
pub fn secret_pubkey(secret_hex: &str) -> Result<String, JsError> {
    Ok(keypair(secret_hex)?.x_only_public_key().0.to_string())
}

/// A UTXO as esplora returns it, plus the address we queried.
#[derive(Deserialize)]
struct JsUtxo {
    txid: String,
    vout: u32,
    value: u64,
    address: String,
}

#[derive(Serialize)]
struct SignedTx {
    hex: String,
    txid: String,
}

#[wasm_bindgen]
pub fn wallet_send(seed_hex: &str, utxos: JsValue, to_address: &str, amount_sat: u64) -> JsResult {
    let w = wallet(seed_hex)?;
    let utxos: Vec<JsUtxo> = from_js(utxos)?;
    let utxos = utxos
        .into_iter()
        .map(|u| {
            Ok(Utxo {
                outpoint: OutPoint { txid: txid(&u.txid)?, vout: u.vout },
                value: Amount::from_sat(u.value),
                script_pubkey: address(&u.address)?,
            })
        })
        .collect::<Result<Vec<_>, JsError>>()?;
    signed(w.send(&utxos, address(to_address)?, Amount::from_sat(amount_sat)).map_err(err)?)
}

// ---------- vault ----------

/// Everything needed to rebuild a vault. Shareable: the thief gets this plus
/// the hot secret.
#[derive(Clone, Serialize, Deserialize)]
struct JsVaultParams {
    amount_sat: u64,
    hot_pubkey: String,
    cold_pubkey: String,
    delay: u16,
    fee_sat: u64,
}

fn vault(params: JsValue) -> Result<Vault, JsError> {
    let p: JsVaultParams = from_js(params)?;
    Vault::new(VaultParams {
        amount: Amount::from_sat(p.amount_sat),
        hot: xonly(&p.hot_pubkey)?,
        cold: xonly(&p.cold_pubkey)?,
        delay: p.delay,
        fee: Amount::from_sat(p.fee_sat),
    })
    .map_err(err)
}

#[wasm_bindgen]
pub fn vault_address(params: JsValue) -> Result<String, JsError> {
    Ok(vault(params)?.address(NETWORK).to_string())
}

/// The tree for the visualizer; txids are filled in once `outpoint` is known.
#[wasm_bindgen]
pub fn vault_view(params: JsValue, vault_outpoint: Option<String>) -> JsResult {
    let v = vault(params)?;
    let op = vault_outpoint.as_deref().map(outpoint).transpose()?;
    to_js(&v.contract.view(NETWORK, op))
}

#[wasm_bindgen]
pub fn vault_unvault_outpoint(params: JsValue, vault_outpoint: &str) -> Result<String, JsError> {
    Ok(vault(params)?.unvault_outpoint(outpoint(vault_outpoint)?).to_string())
}

#[wasm_bindgen]
pub fn vault_start(params: JsValue, vault_outpoint: &str) -> JsResult {
    signed(vault(params)?.start_withdrawal(outpoint(vault_outpoint)?).map_err(err)?)
}

#[wasm_bindgen]
pub fn vault_to_cold(params: JsValue, vault_outpoint: &str) -> JsResult {
    signed(vault(params)?.to_cold(outpoint(vault_outpoint)?).map_err(err)?)
}

#[wasm_bindgen]
pub fn vault_panic(params: JsValue, vault_outpoint: &str) -> JsResult {
    let v = vault(params)?;
    let unvault = v.unvault_outpoint(outpoint(vault_outpoint)?);
    signed(v.panic(unvault).map_err(err)?)
}

#[wasm_bindgen]
pub fn vault_withdraw_hot(
    params: JsValue,
    vault_outpoint: &str,
    to_address: &str,
    hot_secret_hex: &str,
) -> JsResult {
    let v = vault(params)?;
    let unvault = v.unvault_outpoint(outpoint(vault_outpoint)?);
    let tx =
        v.withdraw_hot(unvault, address(to_address)?, &keypair(hot_secret_hex)?).map_err(err)?;
    signed(tx)
}

// ---------- inheritance ----------

#[derive(Clone, Serialize, Deserialize)]
struct JsHeir {
    label: String,
    address: String,
    share: u32,
}

#[derive(Clone, Serialize, Deserialize)]
struct JsInheritanceParams {
    amount_sat: u64,
    owner_pubkey: String,
    heirs: Vec<JsHeir>,
    delay: u16,
    fee_sat: u64,
}

fn inheritance(params: JsValue) -> Result<Inheritance, JsError> {
    let p: JsInheritanceParams = from_js(params)?;
    let heirs = p
        .heirs
        .iter()
        .map(|h| {
            Ok(Heir { label: h.label.clone(), script_pubkey: address(&h.address)?, share: h.share })
        })
        .collect::<Result<_, JsError>>()?;
    Inheritance::new(InheritanceParams {
        amount: Amount::from_sat(p.amount_sat),
        owner: xonly(&p.owner_pubkey)?,
        heirs,
        delay: p.delay,
        fee: Amount::from_sat(p.fee_sat),
    })
    .map_err(err)
}

#[wasm_bindgen]
pub fn inheritance_address(params: JsValue) -> Result<String, JsError> {
    Ok(inheritance(params)?.address(NETWORK).to_string())
}

#[wasm_bindgen]
pub fn inheritance_view(params: JsValue, outpoint_str: Option<String>) -> JsResult {
    let i = inheritance(params)?;
    let op = outpoint_str.as_deref().map(outpoint).transpose()?;
    to_js(&i.contract.view(NETWORK, op))
}

#[derive(Serialize)]
struct KeepAlive {
    hex: String,
    txid: String,
    /// Parameters of the renewed inheritance, at output 0 of `txid`.
    next_params: JsInheritanceParams,
}

/// "I'm alive": moves the coins into a fresh inheritance (same heirs).
#[wasm_bindgen]
pub fn inheritance_keep_alive(params: JsValue, outpoint_str: &str, seed_hex: &str) -> JsResult {
    let mut p: JsInheritanceParams = from_js(params.clone())?;
    let i = inheritance(params)?;
    let (tx, next) = i
        .keep_alive(outpoint(outpoint_str)?, &wallet(seed_hex)?.keypair(Role::Main))
        .map_err(err)?;
    p.amount_sat = next.params.amount.to_sat();
    to_js(&KeepAlive {
        hex: serialize_hex(&tx),
        txid: tx.compute_txid().to_string(),
        next_params: p,
    })
}

#[wasm_bindgen]
pub fn inheritance_withdraw(
    params: JsValue,
    outpoint_str: &str,
    seed_hex: &str,
    to_address: &str,
) -> JsResult {
    let i = inheritance(params)?;
    let tx = i
        .withdraw(
            outpoint(outpoint_str)?,
            address(to_address)?,
            &wallet(seed_hex)?.keypair(Role::Main),
        )
        .map_err(err)?;
    signed(tx)
}

#[wasm_bindgen]
pub fn inheritance_claim(params: JsValue, outpoint_str: &str) -> JsResult {
    signed(inheritance(params)?.claim(outpoint(outpoint_str)?).map_err(err)?)
}

// ---------- subscription ----------

#[derive(Clone, Serialize, Deserialize)]
struct JsSubscriptionParams {
    payment_sat: u64,
    count: u16,
    every: u16,
    receiver_address: String,
    payer_pubkey: String,
    fee_sat: u64,
}

fn subscription(params: JsValue) -> Result<Subscription, JsError> {
    let p: JsSubscriptionParams = from_js(params)?;
    Subscription::new(SubscriptionParams {
        payment: Amount::from_sat(p.payment_sat),
        count: p.count,
        every: p.every,
        receiver: address(&p.receiver_address)?,
        payer: xonly(&p.payer_pubkey)?,
        fee: Amount::from_sat(p.fee_sat),
    })
    .map_err(err)
}

/// Total to fund: every payment plus one fee per transaction.
#[wasm_bindgen]
pub fn subscription_amount(params: JsValue) -> Result<u64, JsError> {
    Ok(subscription(params)?.amount().to_sat())
}

#[wasm_bindgen]
pub fn subscription_address(params: JsValue) -> Result<String, JsError> {
    Ok(subscription(params)?.address(NETWORK).to_string())
}

#[wasm_bindgen]
pub fn subscription_view(params: JsValue, funding: Option<String>) -> JsResult {
    let s = subscription(params)?;
    let op = funding.as_deref().map(outpoint).transpose()?;
    to_js(&s.contract.view(NETWORK, op))
}

#[wasm_bindgen]
pub fn subscription_pay(params: JsValue, k: u16, funding: &str) -> JsResult {
    signed(subscription(params)?.pay(k, outpoint(funding)?).map_err(err)?)
}

#[wasm_bindgen]
pub fn subscription_cancel(
    params: JsValue,
    k: u16,
    funding: &str,
    seed_hex: &str,
    to_address: &str,
) -> JsResult {
    let s = subscription(params)?;
    let tx = s
        .cancel(k, outpoint(funding)?, address(to_address)?, &wallet(seed_hex)?.keypair(Role::Main))
        .map_err(err)?;
    signed(tx)
}

// ---------- helpers ----------

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn to_js<T: Serialize>(v: &T) -> JsResult {
    // Plain objects instead of JS Maps, so the result is easy to use from TS.
    let ser = serde_wasm_bindgen::Serializer::json_compatible();
    v.serialize(&ser).map_err(err)
}

fn from_js<T: DeserializeOwned>(v: JsValue) -> Result<T, JsError> {
    serde_wasm_bindgen::from_value(v).map_err(err)
}

fn signed(tx: Transaction) -> JsResult {
    to_js(&SignedTx { hex: serialize_hex(&tx), txid: tx.compute_txid().to_string() })
}

fn wallet(seed_hex: &str) -> Result<Wallet, JsError> {
    let seed = <[u8; 32]>::from_hex(seed_hex).map_err(|_| err("seed must be 32 bytes of hex"))?;
    Ok(Wallet::from_seed(seed))
}

fn keypair(secret_hex: &str) -> Result<Keypair, JsError> {
    let sk = SecretKey::from_str(secret_hex.trim()).map_err(|_| err("invalid secret key"))?;
    Ok(Keypair::from_secret_key(&SECP, &sk))
}

fn xonly(s: &str) -> Result<XOnlyPublicKey, JsError> {
    XOnlyPublicKey::from_str(s).map_err(|_| err(format!("invalid public key: {s}")))
}

fn txid(s: &str) -> Result<Txid, JsError> {
    Txid::from_str(s).map_err(|_| err(format!("invalid txid: {s}")))
}

fn outpoint(s: &str) -> Result<OutPoint, JsError> {
    OutPoint::from_str(s).map_err(|_| err(format!("invalid outpoint: {s}")))
}

fn address(s: &str) -> Result<ScriptBuf, JsError> {
    Address::from_str(s.trim())
        .map_err(|e| err(format!("invalid address: {e}")))?
        .require_network(NETWORK)
        .map(|a| a.script_pubkey())
        .map_err(|_| err("not a regtest address"))
}
