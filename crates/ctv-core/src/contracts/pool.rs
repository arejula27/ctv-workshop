//! Payment pool: several people share one UTXO and anyone can leave at any
//! time, without asking the others.
//!
//! ```text
//! pool(S): one leaf per member i:   [ <pk_i> CHECKSIGVERIFY <H(exit_i)> CTV ]
//! exit_i, |S| > 2:  pool(S \ i) gets (|S|-1)·share,  i gets share - fee
//! exit_i, |S| = 2:  i gets share - fee,  the other gets share  (pool dissolves)
//! ```
//!
//! Design from stutxo/op_ctv_payment_pool (MIT), plus a signature per leaf so
//! nobody can push someone else out. Every exit order is precomputed, so the
//! tree has |S|! paths: tables are capped at [`MAX_MEMBERS`].

use bitcoin::{Amount, Network, OutPoint, ScriptBuf, Transaction, XOnlyPublicKey, key::Keypair};

use crate::{
    Error,
    tree::{CompiledContract, Contract, Leaf, Output, Template},
    wallet::DUST,
};

pub const MAX_MEMBERS: usize = 6;

#[derive(Clone, Debug)]
pub struct PoolMember {
    pub label: String,
    pub pubkey: XOnlyPublicKey,
    /// Where this member's share goes when they leave.
    pub payout: ScriptBuf,
}

#[derive(Clone, Debug)]
pub struct PoolParams {
    pub members: Vec<PoolMember>,
    pub share: Amount,
    /// Paid by whoever leaves.
    pub fee: Amount,
}

#[derive(Clone, Debug)]
pub struct Pool {
    pub params: PoolParams,
    pub contract: CompiledContract,
}

impl Pool {
    pub fn new(params: PoolParams) -> Result<Self, Error> {
        let n = params.members.len();
        if !(2..=MAX_MEMBERS).contains(&n) {
            return Err(Error::Amount(format!("a pool needs 2..={MAX_MEMBERS} members")));
        }
        if params.share < params.fee + DUST {
            return Err(Error::Amount(format!(
                "each share must be at least {}",
                params.fee + DUST
            )));
        }
        let mut keys: Vec<_> = params.members.iter().map(|m| m.pubkey).collect();
        keys.sort();
        keys.dedup();
        if keys.len() != n {
            return Err(Error::Amount("every member needs a different key".into()));
        }
        let all: Vec<usize> = (0..n).collect();
        let contract = pool_contract(&params, &all).compile(params.share * n as u64)?;
        Ok(Self { params, contract })
    }

    pub fn amount(&self) -> Amount {
        self.contract.value
    }

    pub fn address(&self, network: Network) -> bitcoin::Address {
        self.contract.address(network)
    }

    /// The pool after `exited` (member indexes, in the order they left), and who is still in.
    pub fn current(&self, exited: &[usize]) -> Result<(&CompiledContract, Vec<usize>), Error> {
        let mut inside: Vec<usize> = (0..self.params.members.len()).collect();
        let mut c = &self.contract;
        for &who in exited {
            let leaf = inside.iter().position(|&m| m == who).ok_or(Error::Leaf(who))?;
            if inside.len() == 2 {
                return Err(Error::Amount("the pool has already dissolved".into()));
            }
            let exit = c.leaves[leaf].template.as_ref().expect("pool leaves are CTV");
            c = exit.children[0].as_ref().expect("exit keeps the rest in a pool");
            inside.remove(leaf);
        }
        Ok((c, inside))
    }

    /// `who` leaves the pool that results from `exited`, sitting at `outpoint`.
    /// If others stay, their new pool is output 0 of the returned transaction.
    pub fn exit(
        &self,
        exited: &[usize],
        who: usize,
        outpoint: OutPoint,
        key: &Keypair,
    ) -> Result<Transaction, Error> {
        let (c, inside) = self.current(exited)?;
        let leaf = inside.iter().position(|&m| m == who).ok_or(Error::Leaf(who))?;
        c.spend_ctv(leaf, outpoint, Some(key))
    }
}

/// The pool shared by `inside` (indexes into `params.members`).
fn pool_contract(params: &PoolParams, inside: &[usize]) -> Contract {
    let n = inside.len() as u64;
    let leaves = inside
        .iter()
        .map(|&i| {
            let me = &params.members[i];
            let mine = Output::Pay {
                label: me.label.clone(),
                script_pubkey: me.payout.clone(),
                value: params.share - params.fee,
            };
            let rest: Vec<usize> = inside.iter().copied().filter(|&m| m != i).collect();
            let outputs = if rest.len() == 1 {
                let other = &params.members[rest[0]];
                vec![
                    mine,
                    Output::Pay {
                        label: other.label.clone(),
                        script_pubkey: other.payout.clone(),
                        value: params.share,
                    },
                ]
            } else {
                vec![
                    Output::Contract {
                        value: params.share * (n - 1),
                        contract: pool_contract(params, &rest),
                    },
                    mine,
                ]
            };
            Leaf::Ctv {
                label: format!("Sale {}", me.label),
                csv: None,
                signer: Some(me.pubkey),
                next: Template { label: format!("Salida de {}", me.label), outputs },
            }
        })
        .collect();
    Contract { label: format!("Pool de {}", inside.len()), leaves }
}

#[cfg(test)]
mod tests {
    use bitcoin::{Txid, hashes::Hash};

    use super::*;
    use crate::taproot::SECP;

    fn key(n: u8) -> Keypair {
        Keypair::from_seckey_slice(&SECP, &[n; 32]).unwrap()
    }

    fn pool(n: u8) -> Pool {
        Pool::new(PoolParams {
            members: (1..=n)
                .map(|i| PoolMember {
                    label: format!("m{i}"),
                    pubkey: key(i).x_only_public_key().0,
                    payout: ScriptBuf::new_p2tr(&SECP, key(i).x_only_public_key().0, None),
                })
                .collect(),
            share: Amount::from_sat(100_000),
            fee: Amount::from_sat(500),
        })
        .unwrap()
    }

    fn op() -> OutPoint {
        OutPoint { txid: Txid::all_zeros(), vout: 0 }
    }

    #[test]
    fn exits_in_any_order_keep_the_rest_inside() {
        let p = pool(4);
        assert_eq!(p.amount(), Amount::from_sat(400_000));
        // m3 leaves first, then m1
        let t1 = p.exit(&[], 2, op(), &key(3)).unwrap();
        assert_eq!(t1.output[0].value, Amount::from_sat(300_000));
        assert_eq!(t1.output[1].value, Amount::from_sat(99_500));
        let (next, inside) = p.current(&[2]).unwrap();
        assert_eq!(inside, vec![0, 1, 3]);
        assert_eq!(t1.output[0].script_pubkey, next.script_pubkey());

        let t2 = p.exit(&[2], 0, OutPoint { txid: t1.compute_txid(), vout: 0 }, &key(1)).unwrap();
        assert_eq!(t2.output[0].value, Amount::from_sat(200_000));
        // two left: the next exit dissolves it, the last one gets a full share
        let t3 =
            p.exit(&[2, 0], 3, OutPoint { txid: t2.compute_txid(), vout: 0 }, &key(4)).unwrap();
        assert_eq!(t3.output[0].value, Amount::from_sat(99_500));
        assert_eq!(t3.output[1].value, Amount::from_sat(100_000));
        assert!(p.current(&[2, 0, 3]).is_err());
    }

    #[test]
    fn nobody_can_push_someone_else_out() {
        let p = pool(3);
        assert!(matches!(p.exit(&[], 1, op(), &key(1)), Err(Error::WrongKey)));
    }

    #[test]
    fn six_members_compile() {
        let p = pool(6);
        assert_eq!(p.contract.leaves.len(), 6);
        assert!(Pool::new(PoolParams { members: vec![], ..p.params }).is_err());
    }
}
