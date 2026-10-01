//! Shared helpers for the regtest integration tests.
//!
//! Tests are skipped unless `CTV_RPC_URL` is set; run them with
//! `infra/scripts/test-regtest.sh`.
#![allow(dead_code)]

use std::sync::Mutex;

use base64::Engine;
use bitcoin::{
    Amount, OutPoint, Transaction, Txid, consensus::encode::serialize_hex, key::Keypair,
};
use ctv_core::taproot::SECP;
use serde_json::{Value, json};

pub struct Node {
    agent: ureq::Agent,
    url: String,
    auth: String,
}

// Tests share one chain; serialize them so block counts stay predictable.
pub static LOCK: Mutex<()> = Mutex::new(());

impl Node {
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("CTV_RPC_URL").ok()?;
        let user = std::env::var("CTV_RPC_USER").expect("CTV_RPC_USER");
        let pass = std::env::var("CTV_RPC_PASS").expect("CTV_RPC_PASS");
        let auth = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
        let agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        Some(Self { agent, url, auth: format!("Basic {auth}") })
    }

    pub fn call(&self, path: &str, method: &str, params: Value) -> Result<Value, String> {
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

    pub fn rpc(&self, method: &str, params: Value) -> Value {
        self.call("", method, params).unwrap_or_else(|e| panic!("{method}: {e}"))
    }

    pub fn wallet(&self, method: &str, params: Value) -> Value {
        self.call("/wallet/faucet", method, params).unwrap_or_else(|e| panic!("{method}: {e}"))
    }

    pub fn mine(&self, n: u64) {
        let addr = self.wallet("getnewaddress", json!([]));
        self.rpc("generatetoaddress", json!([n, addr]));
    }

    /// Sends `amount` to `address` from the faucet wallet and confirms it.
    pub fn fund(&self, address: &str, amount: Amount) -> OutPoint {
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

    pub fn broadcast(&self, tx: &Transaction) -> Result<Txid, String> {
        self.call("", "sendrawtransaction", json!([serialize_hex(tx)]))
            .map(|v| v.as_str().unwrap().parse().unwrap())
    }

    pub fn confirmations(&self, txid: Txid) -> u64 {
        self.rpc("getrawtransaction", json!([txid.to_string(), 1]))["confirmations"]
            .as_u64()
            .unwrap_or(0)
    }
}

pub fn key(n: u8) -> Keypair {
    Keypair::from_seckey_slice(&SECP, &[n; 32]).unwrap()
}

/// Confirmed balance of an address, from the node's UTXO set.
pub fn balance(node: &Node, address: &str) -> Amount {
    let r = node.rpc("scantxoutset", json!(["start", [format!("addr({address})")]]));
    Amount::from_btc(r["total_amount"].as_f64().unwrap_or(0.0)).unwrap()
}

#[macro_export]
macro_rules! node_or_skip {
    () => {
        match $crate::common::Node::from_env() {
            Some(n) => n,
            None => {
                eprintln!("CTV_RPC_URL not set, skipping regtest test");
                return;
            }
        }
    };
}
