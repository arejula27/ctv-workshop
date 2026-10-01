//! Inheritance paths against a real Bitcoin Inquisition regtest node.

mod common;

use bitcoin::{Amount, Network, OutPoint, ScriptBuf};
use common::{LOCK, Node, key};
use ctv_core::{
    contracts::inheritance::{Heir, Inheritance, InheritanceParams},
    taproot::SECP,
};

const DELAY: u16 = 10;

fn spk(n: u8) -> ScriptBuf {
    ScriptBuf::new_p2tr(&SECP, key(n).x_only_public_key().0, None)
}

fn funded(node: &Node, sats: u64) -> (Inheritance, OutPoint) {
    let i = Inheritance::new(InheritanceParams {
        amount: Amount::from_sat(sats),
        owner: key(1).x_only_public_key().0,
        heirs: vec![
            Heir { label: "Hija".into(), script_pubkey: spk(2), share: 1 },
            Heir { label: "Hijo".into(), script_pubkey: spk(3), share: 1 },
        ],
        delay: DELAY,
        fee: Amount::from_sat(1_000),
    })
    .unwrap();
    let op = node.fund(&i.address(Network::Regtest).to_string(), i.params.amount);
    (i, op)
}

#[test]
fn heirs_wait_and_keep_alive_restarts_the_clock() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (i, op) = funded(&node, 200_001);

    let early = node.broadcast(&i.claim(op).unwrap()).unwrap_err();
    assert!(early.contains("non-BIP68-final"), "{early}");

    // Almost there... and the owner shows up: the clock restarts.
    node.mine(DELAY as u64 - 2);
    let (alive, next) = i.keep_alive(op, &key(1)).unwrap();
    let alive_txid = node.broadcast(&alive).unwrap();
    node.mine(1);
    let next_op = OutPoint { txid: alive_txid, vout: 0 };

    let early = node.broadcast(&next.claim(next_op).unwrap()).unwrap_err();
    assert!(early.contains("non-BIP68-final"), "{early}");
    node.mine(DELAY as u64);
    let split = node.broadcast(&next.claim(next_op).unwrap()).unwrap();
    node.mine(1);
    assert!(node.confirmations(split) >= 1);
}

#[test]
fn owner_takes_it_back() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (i, op) = funded(&node, 200_002);
    let txid = node.broadcast(&i.withdraw(op, spk(1), &key(1)).unwrap()).unwrap();
    node.mine(1);
    assert!(node.confirmations(txid) >= 1);
    // Someone else's key can't.
    assert!(i.withdraw(op, spk(9), &key(9)).is_err());
}
