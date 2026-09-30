//! Vault paths against a real Bitcoin Inquisition regtest node.
//!
//! Skipped unless `CTV_RPC_URL` is set. Run with `infra/scripts/test-regtest.sh`,
//! which points it at the compose stack's debug RPC port.

use std::sync::Mutex;

use base64::Engine;
use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Transaction, Txid, consensus::encode::serialize_hex,
    key::Keypair,
};
use ctv_core::{
    contracts::vault::{Vault, VaultParams},
    taproot::SECP,
};
use serde_json::{Value, json};

const DELAY: u16 = 20;

struct Node {
    agent: ureq::Agent,
    url: String,
    auth: String,
}

// Tests share one chain; serialize them so block counts stay predictable.
static LOCK: Mutex<()> = Mutex::new(());

impl Node {
    fn from_env() -> Option<Self> {
        let url = std::env::var("CTV_RPC_URL").ok()?;
        let user = std::env::var("CTV_RPC_USER").expect("CTV_RPC_USER");
        let pass = std::env::var("CTV_RPC_PASS").expect("CTV_RPC_PASS");
        let auth = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
        let agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        Some(Self { agent, url, auth: format!("Basic {auth}") })
    }

    fn call(&self, path: &str, method: &str, params: Value) -> Result<Value, String> {
        let body = json!({ "jsonrpc": "2.0", "id": 0, "method": method, "params": params });
        let mut resp = self
            .agent
            .post(format!("{}{path}", self.url))
            .header("Authorization", &self.auth)
            .send_json(&body)
            .map_err(|e| e.to_string())?;
        let v: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
        match v.get("error").filter(|e| !e.is_null()) {
            Some(err) => Err(err["message"].as_str().unwrap_or_default().to_owned()),
            None => Ok(v["result"].clone()),
        }
    }

    fn rpc(&self, method: &str, params: Value) -> Value {
        self.call("", method, params).unwrap_or_else(|e| panic!("{method}: {e}"))
    }

    fn wallet(&self, method: &str, params: Value) -> Value {
        self.call("/wallet/faucet", method, params).unwrap_or_else(|e| panic!("{method}: {e}"))
    }

    fn mine(&self, n: u64) {
        let addr = self.wallet("getnewaddress", json!([]));
        self.rpc("generatetoaddress", json!([n, addr]));
    }

    /// Sends `amount` to `address` from the faucet wallet and confirms it.
    fn fund(&self, address: &str, amount: Amount) -> OutPoint {
        let txid = self.wallet("sendtoaddress", json!([address, amount.to_btc()]));
        let tx = self.rpc("getrawtransaction", json!([txid, 1]));
        let vout = tx["vout"]
            .as_array()
            .unwrap()
            .iter()
            .position(|o| o["scriptPubKey"]["address"] == address)
            .unwrap();
        self.mine(1);
        OutPoint { txid: txid.as_str().unwrap().parse().unwrap(), vout: vout as u32 }
    }

    fn broadcast(&self, tx: &Transaction) -> Result<Txid, String> {
        self.call("", "sendrawtransaction", json!([serialize_hex(tx)]))
            .map(|v| v.as_str().unwrap().parse().unwrap())
    }

    fn confirmations(&self, txid: Txid) -> u64 {
        self.rpc("getrawtransaction", json!([txid.to_string(), 1]))["confirmations"]
            .as_u64()
            .unwrap_or(0)
    }
}

fn key(n: u8) -> Keypair {
    Keypair::from_seckey_slice(&SECP, &[n; 32]).unwrap()
}

/// A fresh vault funded on chain. The amount varies per test so addresses differ.
fn funded_vault(node: &Node, sats: u64) -> (Vault, OutPoint) {
    let vault = Vault::new(VaultParams {
        amount: Amount::from_sat(sats),
        hot: key(1).x_only_public_key().0,
        cold: key(2).x_only_public_key().0,
        delay: DELAY,
        fee: Amount::from_sat(1_000),
    })
    .unwrap();
    let outpoint = node.fund(&vault.address(Network::Regtest).to_string(), vault.params.amount);
    (vault, outpoint)
}

macro_rules! node_or_skip {
    () => {
        match Node::from_env() {
            Some(n) => n,
            None => {
                eprintln!("CTV_RPC_URL not set, skipping regtest test");
                return;
            }
        }
    };
}

#[test]
fn vault_straight_to_cold() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (vault, op) = funded_vault(&node, 100_001);

    let txid = node.broadcast(&vault.to_cold(op).unwrap()).unwrap();
    node.mine(1);
    assert!(node.confirmations(txid) >= 1);
}

#[test]
fn vault_panic_button() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (vault, op) = funded_vault(&node, 100_002);

    let start = node.broadcast(&vault.start_withdrawal(op).unwrap()).unwrap();
    assert_eq!(start, vault.unvault_outpoint(op).txid, "txid known before broadcasting");
    node.mine(1);

    let panic = node.broadcast(&vault.panic(vault.unvault_outpoint(op)).unwrap()).unwrap();
    node.mine(1);
    assert!(node.confirmations(panic) >= 1);
}

#[test]
fn vault_hot_key_must_wait() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (vault, op) = funded_vault(&node, 100_003);
    node.broadcast(&vault.start_withdrawal(op).unwrap()).unwrap();
    node.mine(1);

    let to = ScriptBuf::new_p2tr(&SECP, key(3).x_only_public_key().0, None);
    let hot = vault.withdraw_hot(vault.unvault_outpoint(op), to, &key(1)).unwrap();
    let early = node.broadcast(&hot).unwrap_err();
    assert!(early.contains("non-BIP68-final"), "{early}");

    node.mine(DELAY as u64);
    let txid = node.broadcast(&hot).unwrap();
    node.mine(1);
    assert!(node.confirmations(txid) >= 1);
}

#[test]
fn vault_rejects_tampered_template() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (vault, op) = funded_vault(&node, 100_004);

    // A thief trying to redirect the cold sweep to their own key.
    let mut tx = vault.to_cold(op).unwrap();
    tx.output[0].script_pubkey = ScriptBuf::new_p2tr(&SECP, key(66).x_only_public_key().0, None);
    let err = node.broadcast(&tx).unwrap_err();
    assert!(err.contains("script-verify-flag-failed"), "{err}");

    // Same with the amount.
    let mut tx = vault.to_cold(op).unwrap();
    tx.output[0].value -= Amount::from_sat(1);
    let err = node.broadcast(&tx).unwrap_err();
    assert!(err.contains("script-verify-flag-failed"), "{err}");
}

/// The browser flow end to end: faucet → participant wallet → vault → thief
/// with the hot key starts a withdrawal → owner presses panic.
#[test]
fn wallet_funds_vault_and_owner_beats_thief() {
    use ctv_core::wallet::{Role, Utxo, Wallet};

    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let alice = Wallet::from_seed([0xa1; 32]);
    let main = alice.address(Role::Main, Network::Regtest);
    let faucet = node.fund(&main.to_string(), Amount::from_sat(1_000_000));
    let utxos = [Utxo {
        outpoint: faucet,
        value: Amount::from_sat(1_000_000),
        script_pubkey: main.script_pubkey(),
    }];

    let vault = Vault::new(VaultParams {
        amount: Amount::from_sat(600_000),
        hot: alice.keypair(Role::Hot).x_only_public_key().0,
        cold: alice.keypair(Role::Cold).x_only_public_key().0,
        delay: DELAY,
        fee: Amount::from_sat(1_000),
    })
    .unwrap();
    let funding = alice.send(&utxos, vault.contract.script_pubkey(), vault.params.amount).unwrap();
    let funding_txid = node.broadcast(&funding).unwrap();
    node.mine(1);
    let vault_op = OutPoint { txid: funding_txid, vout: 0 };

    // Anyone can start the withdrawal; the thief does.
    node.broadcast(&vault.start_withdrawal(vault_op).unwrap()).unwrap();
    node.mine(1);
    // Alice notices and sweeps to her cold key before the delay ends.
    let panic = node.broadcast(&vault.panic(vault.unvault_outpoint(vault_op)).unwrap()).unwrap();
    node.mine(1);
    assert!(node.confirmations(panic) >= 1);

    // The thief, even with the hot key, has nothing left to take.
    let thief_to = ScriptBuf::new_p2tr(&SECP, key(66).x_only_public_key().0, None);
    node.mine(DELAY as u64);
    let hot = vault
        .withdraw_hot(vault.unvault_outpoint(vault_op), thief_to, &alice.keypair(Role::Hot))
        .unwrap();
    assert!(node.broadcast(&hot).is_err());
}
