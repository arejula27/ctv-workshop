//! Payment pool against a real Bitcoin Inquisition regtest node.

mod common;

use bitcoin::{Amount, Network, OutPoint, ScriptBuf, TxOut};
use common::{LOCK, Node, key};
use ctv_core::{
    contracts::pool::{Pool, PoolMember, PoolParams},
    taproot::SECP,
    wallet::{Role, Utxo, Wallet},
};

fn spk(n: u8) -> ScriptBuf {
    ScriptBuf::new_p2tr(&SECP, key(n).x_only_public_key().0, None)
}

fn pool(balances: &[u64]) -> Pool {
    Pool::new(PoolParams {
        members: balances
            .iter()
            .enumerate()
            .map(|(i, &b)| PoolMember {
                label: format!("m{}", i + 1),
                pubkey: key(i as u8 + 1).x_only_public_key().0,
                payout: spk(i as u8 + 1),
                balance: Amount::from_sat(b),
            })
            .collect(),
        fee: Amount::from_sat(500),
    })
    .unwrap()
}

/// Everyone signs `next` and the update is broadcast and confirmed.
fn update(node: &Node, p: &Pool, op: OutPoint, next: &Pool, extra_out: Vec<TxOut>) -> OutPoint {
    let tx = p.update_tx(op, next, &[], extra_out).unwrap();
    let prevouts = p.update_prevouts(&[]);
    let sigs: Vec<_> = (0..p.params.members.len())
        .map(|i| {
            let k = key(i as u8 + 1);
            (k.x_only_public_key().0, p.sign_update(&tx, &prevouts, &k).unwrap())
        })
        .collect();
    let txid = node.broadcast(&p.finalize_update(tx, &sigs).unwrap()).unwrap();
    node.mine(1);
    assert!(node.confirmations(txid) >= 1);
    OutPoint { txid, vout: 0 }
}

#[test]
fn members_leave_in_any_order_without_asking() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = pool(&[100_001, 100_000, 100_000, 100_000]);
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
fn pay_inside_then_leave_with_the_new_balance() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p0 = pool(&[100_002, 100_000, 100_000]);
    let op0 = node.fund(&p0.address(Network::Regtest).to_string(), p0.amount());

    // m1 pays m2 20.000 inside the pool (m1 also pays the fee): everyone signs.
    let p1 = pool(&[79_502, 120_000, 100_000]);
    let op1 = update(&node, &p0, op0, &p1, vec![]);

    // m3 sends 30.000 out of the pool to an outside address.
    let p2 = pool(&[79_502, 120_000, 69_500]);
    let op2 = update(
        &node,
        &p1,
        op1,
        &p2,
        vec![TxOut { value: Amount::from_sat(30_000), script_pubkey: spk(66) }],
    );

    // m2 leaves on their own with what they have now, not what they started with.
    let exit = p2.exit(&[], 1, op2, &key(2)).unwrap();
    assert_eq!(exit.output[1].value, Amount::from_sat(119_500));
    let txid = node.broadcast(&exit).unwrap();
    node.mine(1);
    assert!(node.confirmations(txid) >= 1);
}

#[test]
fn deposit_from_a_wallet_into_the_pool() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p0 = pool(&[100_003, 100_000]);
    let op0 = node.fund(&p0.address(Network::Regtest).to_string(), p0.amount());

    // m1's on-chain wallet has 60.000; they put 50.000 in (change back, fee from the deposit).
    let w = Wallet::from_seed([0xd1; 32]);
    let coin_op =
        node.fund(&w.address(Role::Main, Network::Regtest).to_string(), Amount::from_sat(60_000));
    let coin = Utxo {
        outpoint: coin_op,
        value: Amount::from_sat(60_000),
        script_pubkey: w.script_pubkey(Role::Main),
    };
    let p1 = pool(&[150_003, 100_000]);
    let change =
        TxOut { value: Amount::from_sat(9_500), script_pubkey: w.script_pubkey(Role::Main) };
    let mut tx = p0.update_tx(op0, &p1, std::slice::from_ref(&coin), vec![change]).unwrap();
    let prevouts = p0.update_prevouts(std::slice::from_ref(&coin));
    let sigs: Vec<_> = (1..=2)
        .map(|n| (key(n).x_only_public_key().0, p0.sign_update(&tx, &prevouts, &key(n)).unwrap()))
        .collect();
    tx.input[1].witness = w.sign_inputs(&tx, &prevouts, &[1]).unwrap().remove(0);
    let txid = node.broadcast(&p0.finalize_update(tx, &sigs).unwrap()).unwrap();
    node.mine(1);
    assert!(node.confirmations(txid) >= 1);
}

#[test]
fn a_tampered_exit_is_rejected() {
    let node = node_or_skip!();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = pool(&[100_004, 100_000, 100_000]);
    let op = node.fund(&p.address(Network::Regtest).to_string(), p.amount());

    // m1 leaves but tries to take more than their balance
    let mut tx = p.exit(&[], 0, op, &key(1)).unwrap();
    tx.output[1].value += Amount::from_sat(10_000);
    tx.output[0].value -= Amount::from_sat(10_000);
    let err = node.broadcast(&tx).unwrap_err();
    assert!(err.contains("script-verify-flag-failed"), "{err}");
}
