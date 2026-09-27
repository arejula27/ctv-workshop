//! Event bus pushed to browsers over `/ws` (participants and the `/screen` view).

use std::sync::Arc;

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;

use crate::{
    api::{AppState, Status, status_snapshot},
    chain::MinerSettings,
};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Sent once on connect.
    Hello {
        status: Status,
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
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| forward_events(socket, state))
}

async fn forward_events(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.chain.events.subscribe();

    if let Ok(status) = status_snapshot(&state).await
        && send(&mut socket, &Event::Hello { status }).await.is_err()
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
