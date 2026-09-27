//! Minimal async JSON-RPC client for bitcoind.
//!
//! We don't use `bitcoincore-rpc`: it is blocking and its typed results break
//! with newer Core versions (e.g. `warnings` became an array in v28).

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("bitcoind error {code}: {message}")]
    Node { code: i64, message: String },
    #[error("unexpected response: {0}")]
    Decode(String),
}

impl RpcError {
    pub fn code(&self) -> Option<i64> {
        match self {
            RpcError::Node { code, .. } => Some(*code),
            _ => None,
        }
    }
}

// Error codes from bitcoind's rpc/protocol.h
pub const RPC_IN_WARMUP: i64 = -28;
pub const RPC_WALLET_NOT_FOUND: i64 = -18;
pub const RPC_WALLET_ALREADY_LOADED: i64 = -35;

#[derive(Clone)]
pub struct Rpc {
    http: reqwest::Client,
    url: String,
    user: String,
    pass: String,
}

impl Rpc {
    pub fn new(url: &str, user: &str, pass: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: url.trim_end_matches('/').to_owned(),
            user: user.to_owned(),
            pass: pass.to_owned(),
        }
    }

    /// Same node, scoped to a wallet endpoint.
    pub fn wallet(&self, name: &str) -> Self {
        let base = self.url.split("/wallet/").next().unwrap_or(&self.url);
        Self { url: format!("{base}/wallet/{name}"), ..self.clone() }
    }

    pub async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, RpcError> {
        let body = json!({ "jsonrpc": "2.0", "id": 0, "method": method, "params": params });
        let resp = self
            .http
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.pass))
            .json(&body)
            .send()
            .await?;

        // Core replies with a JSON body on errors too (HTTP 500 for jsonrpc 1.0,
        // 200 for 2.0), so parse the body before looking at the status.
        let status = resp.status();
        let text = resp.text().await?;
        let v: Value = serde_json::from_str(&text)
            .map_err(|_| RpcError::Decode(format!("HTTP {status}: {text}")))?;

        if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
            return Err(RpcError::Node {
                code: err["code"].as_i64().unwrap_or_default(),
                message: err["message"].as_str().unwrap_or_default().to_owned(),
            });
        }
        serde_json::from_value(v["result"].clone()).map_err(|e| RpcError::Decode(e.to_string()))
    }
}
