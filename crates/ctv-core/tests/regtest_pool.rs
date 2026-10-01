//! Payment pool against a real Bitcoin Inquisition regtest node.

mod common;

use bitcoin::{Amount, Network, OutPoint, ScriptBuf};
use common::{LOCK, key};
use ctv_core::{
    contracts::pool::{Pool, PoolMember, PoolParams},
    taproot::SECP,
};

fn pool(share: u64) -> Pool {
    Pool::new(PoolParams {
        members: (1..=4)
            .map(|i| PoolMember {
                label: format!("m{i}"),
                pubkey: key(i).x_only_public_key().0,
                payout: ScriptBuf::new_p2tr(&SECP, key(i).x_only_public_key().0, None),
            })
            .collect(),
        share: Amount::from_sat(share),
        fee: Amount::from_sat(500),
    })
    .unwrap()
}

#[test]
fn members_leave_in_any_order_without_asking() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = pool(100_001);
    let mut op = node.fund(&p.address(Network::Regtest).to_string(), p.amount());

    // m3, then m1, then m4 (which dissolves the pool and pays m2 too)
    let order = [2usize, 0, 3];
    for (n, &who) in order.iter().enumerate() {
        let tx = p.exit(&order[..n], who, op, &key(who as u8 + 1)).unwrap();
        let txid = node.broadcast(&tx).unwrap();
        node.mine(1);
        assert!(node.confirmations(txid) >= 1, "exit of m{}", who + 1);
        op = OutPoint { txid, vout: 0 };
    }
}

#[test]
fn a_tampered_exit_is_rejected() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = pool(100_002);
    let op = node.fund(&p.address(Network::Regtest).to_string(), p.amount());

    // m1 leaves but tries to take more than their share
    let mut tx = p.exit(&[], 0, op, &key(1)).unwrap();
    tx.output[1].value += Amount::from_sat(10_000);
    tx.output[0].value -= Amount::from_sat(10_000);
    let err = node.broadcast(&tx).unwrap_err();
    assert!(err.contains("script-verify-flag-failed"), "{err}");
}
