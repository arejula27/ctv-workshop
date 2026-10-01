//! Subscription paths against a real Bitcoin Inquisition regtest node.

mod common;

use bitcoin::{Amount, Network, OutPoint, ScriptBuf};
use common::{LOCK, Node, key};
use ctv_core::{
    contracts::subscription::{Subscription, SubscriptionParams},
    taproot::SECP,
};

const EVERY: u16 = 3;

fn spk(n: u8) -> ScriptBuf {
    ScriptBuf::new_p2tr(&SECP, key(n).x_only_public_key().0, None)
}

fn funded(node: &Node, payment: u64) -> (Subscription, OutPoint) {
    let s = Subscription::new(SubscriptionParams {
        payment: Amount::from_sat(payment),
        count: 3,
        every: EVERY,
        receiver: spk(2),
        payer: key(1).x_only_public_key().0,
        fee: Amount::from_sat(500),
    })
    .unwrap();
    let op = node.fund(&s.address(Network::Regtest).to_string(), s.amount());
    (s, op)
}

#[test]
fn every_payment_waits_its_turn() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (s, op) = funded(&node, 10_001);

    for k in 1..=3 {
        let early = node.broadcast(&s.pay(k, op).unwrap()).unwrap_err();
        assert!(early.contains("non-BIP68-final"), "payment {k}: {early}");
        node.mine(EVERY as u64 - 1);
        let txid = node.broadcast(&s.pay(k, op).unwrap()).unwrap();
        node.mine(1);
        assert!(node.confirmations(txid) >= 1, "payment {k}");
    }
}

#[test]
fn payer_cancels_and_gets_the_rest() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (s, op) = funded(&node, 10_002);

    node.mine(EVERY as u64 - 1);
    node.broadcast(&s.pay(1, op).unwrap()).unwrap();
    node.mine(1);

    // Cancel at step 2: no waiting needed, and payment 2 can't happen anymore.
    let cancel = node.broadcast(&s.cancel(2, op, spk(1), &key(1)).unwrap()).unwrap();
    node.mine(EVERY as u64);
    assert!(node.confirmations(cancel) >= 1);
    let late = node.broadcast(&s.pay(2, op).unwrap()).unwrap_err();
    assert!(late.contains("missingorspent"), "{late}");
}
