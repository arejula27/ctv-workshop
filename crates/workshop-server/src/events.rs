//! Event bus pushed to browsers over `/ws` (participants and the `/screen` view).

use std::sync::Arc;

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;

use crate::{
    api::{AppState, Status, status_snapshot},
    chain::MinerSettings,
};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Sent once on connect, with the latest feed items so a reloaded
    /// `/screen` isn't empty.
    Hello {
        status: Status,
        recent: Vec<Event>,
    },
    Block {
        height: u64,
        hash: String,
    },
    Faucet {
        address: String,
        amount_sat: u64,
        txid: String,
    },
    Miner {
        settings: MinerSettings,
    },
    Jump {
        blocks: u64,
        height: u64,
    },
    ParticipantJoined {
        name: String,
    },
    VaultRegistered {
        owner: String,
        amount_sat: u64,
        delay: u16,
        outpoint: String,
    },
    ContractRegistered {
        kind: crate::registry::ContractKind,
        owner: String,
        amount_sat: u64,
        id: String,
        /// True when an existing contract moved (an inheritance's "I'm alive").
        renewed: bool,
    },
    PoolUpdated {
        id: String,
        name: String,
        /// Member labels, in seat order.
        members: Vec<String>,
        state: crate::registry::PoolState,
        /// What happened: "created", "joined" or "funded".
        change: String,
        amount_sat: u64,
    },
    Activity {
        who: String,
        kind: ActivityKind,
        /// Owner of the vault it happened to.
        owner: String,
        amount_sat: u64,
        /// The vault's funding outpoint.
        outpoint: String,
        txid: String,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    VaultStart,
    VaultToCold,
    VaultPanic,
    VaultWithdraw,
    TheftStart,
    TheftSuccess,
    InheritanceKeepAlive,
    InheritanceWithdraw,
    InheritanceClaim,
    SubscriptionPay,
    SubscriptionCancel,
    PoolExit,
}

impl Event {
    /// Worth showing in the feed (blocks are shown by the clock instead).
    fn is_feed(&self) -> bool {
        !matches!(self, Event::Hello { .. } | Event::Block { .. })
    }
}

const FEED_LEN: usize = 30;

/// Keeps the last feed items for `Hello`.
pub async fn record_feed(state: Arc<AppState>) {
    let mut rx = state.chain.events.subscribe();
    loop {
        match rx.recv().await {
            Ok(ev) if ev.is_feed() => {
                let mut recent = state.recent.lock().await;
                if recent.len() == FEED_LEN {
                    recent.pop_front();
                }
                recent.push_back(ev);
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| forward_events(socket, state))
}

async fn forward_events(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.chain.events.subscribe();

    let recent = state.recent.lock().await.iter().cloned().collect();
    if let Ok(status) = status_snapshot(&state).await
        && send(&mut socket, &Event::Hello { status, recent }).await.is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(ev) => if send(&mut socket, &ev).await.is_err() { return },
                // A slow client missed events; the next block event resyncs it.
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            },
            // Incoming messages are ignored for now (pool coordinator comes in F7);
            // we only watch for the client going away.
            msg = socket.recv() => match msg {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                Some(Ok(_)) => {}
            },
        }
    }
}

async fn send(socket: &mut WebSocket, ev: &Event) -> Result<(), axum::Error> {
    let json = serde_json::to_string(ev).expect("events serialize");
    socket.send(Message::Text(json.into())).await
}
