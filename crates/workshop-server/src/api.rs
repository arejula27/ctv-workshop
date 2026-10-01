use std::{
    collections::{HashMap, VecDeque},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use bitcoin::{Address, Network};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tracing::warn;

use crate::{
    chain::{Chain, MinerSettings},
    config::{Config, MIN_MINER_INTERVAL},
    events::{self, Event},
    pools,
    registry::Registry,
    room,
};

pub struct AppState {
    pub config: Config,
    pub chain: Arc<Chain>,
    pub registry: Registry,
    /// Latest feed events, replayed to new WebSocket clients.
    pub recent: Mutex<VecDeque<Event>>,
    faucet_last: Mutex<HashMap<String, Instant>>,
}

impl AppState {
    pub fn new(config: Config, chain: Arc<Chain>, registry: Registry) -> Self {
        Self { config, chain, registry, recent: Mutex::default(), faucet_last: Mutex::default() }
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    let admin = Router::new()
        .route("/mine", post(mine))
        .route("/jump", post(jump))
        .route("/miner", put(set_miner))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/status", get(status))
        .route("/api/faucet", post(faucet))
        .route("/api/participants", get(room::participants).post(room::join))
        .route("/api/vaults", get(room::vaults).post(room::register_vault))
        .route("/api/contracts", get(room::contracts).post(room::register_contract))
        .route("/api/pools", get(pools::list).post(pools::create))
        .route("/api/pools/{id}/join", post(pools::join))
        .route("/api/pools/{id}/start", post(pools::start))
        .route("/api/activity", post(room::activity))
        .nest("/api/admin", admin)
        .route("/ws", get(events::ws_handler))
        .with_state(state)
}

// ---------- errors ----------

pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        let e = e.into();
        warn!("internal error: {e:#}");
        Self(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;

// ---------- public ----------

/// Normalized regtest address, or a 400.
pub fn regtest_address(s: &str) -> Result<String, ApiError> {
    Ok(Address::from_str(s.trim())
        .map_err(|e| ApiError::bad_request(format!("invalid address: {e}")))?
        .require_network(Network::Regtest)
        .map_err(|_| ApiError::bad_request("not a regtest address"))?
        .to_string())
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub network: &'static str,
    pub height: u64,
    pub miner: MinerSettings,
    pub next_block_in_ms: Option<u64>,
    pub faucet_amount_sat: u64,
    /// Simulated calendar: block `h` happens at `sim_epoch_s + (h - 1) * sim_block_s`.
    pub sim_epoch_s: u64,
    pub sim_block_s: u64,
}

pub async fn status_snapshot(state: &AppState) -> anyhow::Result<Status> {
    Ok(Status {
        network: "regtest",
        height: state.chain.height().await?,
        miner: *state.chain.miner.borrow(),
        next_block_in_ms: state.chain.next_block_in_ms(),
        faucet_amount_sat: state.config.faucet_amount.to_sat(),
        sim_epoch_s: state.chain.sim_epoch_s(),
        sim_block_s: state.config.sim_block_s,
    })
}

async fn status(State(state): State<Arc<AppState>>) -> ApiResult<Status> {
    Ok(Json(status_snapshot(&state).await?))
}

#[derive(Deserialize)]
struct FaucetReq {
    address: String,
}

#[derive(Serialize)]
struct FaucetResp {
    txid: String,
    amount_sat: u64,
    /// True when the miner is paused and we mined a block so the coins confirm.
    mined: bool,
}

async fn faucet(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FaucetReq>,
) -> ApiResult<FaucetResp> {
    let address = regtest_address(&req.address)?;

    {
        let mut last = state.faucet_last.lock().await;
        let cooldown = state.config.faucet_cooldown;
        if let Some(wait) = last.get(&address).and_then(|t| cooldown.checked_sub(t.elapsed())) {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                format!("wait {}s before asking again", wait.as_secs() + 1),
            ));
        }
        last.insert(address.clone(), Instant::now());
    }

    let amount_sat = state.config.faucet_amount.to_sat();
    let txid = state.chain.send(&address, amount_sat).await?;
    let mined = state.chain.miner.borrow().paused;
    if mined {
        state.chain.mine(1).await?;
    }
    let _ = state.chain.events.send(Event::Faucet { address, amount_sat, txid: txid.clone() });
    Ok(Json(FaucetResp { txid, amount_sat, mined }))
}

// ---------- admin ----------

async fn require_admin(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let expected = format!("Bearer {}", state.config.admin_token);
    match headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        Some(got) if got == expected => Ok(next.run(req).await),
        _ => Err(ApiError(StatusCode::UNAUTHORIZED, "admin token required".into())),
    }
}

#[derive(Deserialize)]
struct BlocksReq {
    #[serde(default = "one")]
    blocks: u64,
}

fn one() -> u64 {
    1
}

#[derive(Serialize)]
struct HeightResp {
    height: u64,
}

async fn mine(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BlocksReq>,
) -> ApiResult<HeightResp> {
    let blocks = check_blocks(&state, req.blocks)?;
    Ok(Json(HeightResp { height: state.chain.mine(blocks).await? }))
}

/// Same as `mine` but announced as a time jump, so `/screen` can show it.
async fn jump(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BlocksReq>,
) -> ApiResult<HeightResp> {
    let blocks = check_blocks(&state, req.blocks)?;
    let height = state.chain.mine(blocks).await?;
    let _ = state.chain.events.send(Event::Jump { blocks, height });
    Ok(Json(HeightResp { height }))
}

fn check_blocks(state: &AppState, blocks: u64) -> Result<u64, ApiError> {
    if blocks == 0 || blocks > state.config.max_jump {
        return Err(ApiError::bad_request(format!("blocks must be 1..={}", state.config.max_jump)));
    }
    Ok(blocks)
}

#[derive(Deserialize)]
struct MinerReq {
    paused: Option<bool>,
    interval_s: Option<u64>,
}

async fn set_miner(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MinerReq>,
) -> ApiResult<MinerSettings> {
    let mut settings = *state.chain.miner.borrow();
    if let Some(paused) = req.paused {
        settings.paused = paused;
    }
    if let Some(interval_s) = req.interval_s {
        if Duration::from_secs(interval_s) < MIN_MINER_INTERVAL {
            return Err(ApiError::bad_request(format!(
                "interval_s must be >= {}",
                MIN_MINER_INTERVAL.as_secs()
            )));
        }
        settings.interval_s = interval_s;
    }
    state.chain.miner.send_replace(settings);
    let _ = state.chain.events.send(Event::Miner { settings });
    Ok(Json(settings))
}
