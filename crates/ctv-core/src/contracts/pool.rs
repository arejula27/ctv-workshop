//! Payment pool: several people share one UTXO. Anyone can leave at any time
//! with their balance, without asking; changing balances needs everyone.
//!
//! ```text
//! pool(S): [ <pk_1> CHECKSIG … <pk_n> CHECKSIGADD <n> NUMEQUAL ]      everyone: update to a new state
//!          one leaf per member i: [ <pk_i> CHECKSIGVERIFY <H(exit_i)> CTV ]
//! exit_i, |S| > 2:  pool(S \ i) keeps the others' balances,  i gets balance_i - fee
//! exit_i, |S| = 2:  i gets balance_i - fee,  the other gets their balance  (dissolves)
//! ```
//!
//! Exits are from stutxo/op_ctv_payment_pool (MIT), plus a signature per leaf so
//! nobody can push someone else out, and per-member balances. A cooperative
//! update (pay a member, send out, deposit in) spends the multisig leaf into the
//! next state; each state precomputes every exit order (|S|! paths), so tables
//! are capped at [`MAX_MEMBERS`].

use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, XOnlyPublicKey,
    absolute::LockTime, key::Keypair, secp256k1::schnorr::Signature, transaction::Version,
};

use crate::{
    Error,
    tree::{CompiledContract, Contract, Leaf, Output, Template},
    wallet::{DUST, Utxo},
};

pub const MAX_MEMBERS: usize = 6;
/// The cooperative (everyone signs) leaf of every pool contract.
pub const COOP: usize = 0;

#[derive(Clone, Debug, PartialEq)]
pub struct PoolMember {
    pub label: String,
    pub pubkey: XOnlyPublicKey,
    /// Where this member's balance goes when they leave.
    pub payout: ScriptBuf,
    pub balance: Amount,
}

#[derive(Clone, Debug)]
pub struct PoolParams {
    pub members: Vec<PoolMember>,
    /// Paid by whoever leaves, and by whoever starts an update.
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
        if let Some(m) = params.members.iter().find(|m| m.balance < params.fee + DUST) {
            return Err(Error::Amount(format!(
                "{} needs at least {} in the pool",
                m.label,
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
        let total = params.members.iter().map(|m| m.balance).sum();
        let contract = pool_contract(&params, &all).compile(total)?;
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
            let pos = inside.iter().position(|&m| m == who).ok_or(Error::Leaf(who))?;
            if inside.len() == 2 {
                return Err(Error::Amount("the pool has already dissolved".into()));
            }
            let exit = c.leaves[COOP + 1 + pos].template.as_ref().expect("exit leaves are CTV");
            c = exit.children[0].as_ref().expect("an exit keeps the rest in a pool");
            inside.remove(pos);
        }
        Ok((c, inside))
    }

    /// `who` leaves the pool that results from `exited`, sitting at `outpoint`,
    /// with their current balance. If others stay, their pool is output 0.
    pub fn exit(
        &self,
        exited: &[usize],
        who: usize,
        outpoint: OutPoint,
        key: &Keypair,
    ) -> Result<Transaction, Error> {
        let (c, inside) = self.current(exited)?;
        let pos = inside.iter().position(|&m| m == who).ok_or(Error::Leaf(who))?;
        c.spend_ctv(COOP + 1 + pos, outpoint, Some(key))
    }

    // ---------- cooperative updates ----------

    /// Unsigned update of this pool (at `outpoint`) into `next`: output 0 is the
    /// next state, then `extra_outputs`; input 0 is the pool, then `extra_inputs`
    /// (e.g. a member's coins for a deposit). Whatever is left over is the fee.
    pub fn update_tx(
        &self,
        outpoint: OutPoint,
        next: &Pool,
        extra_inputs: &[Utxo],
        extra_outputs: Vec<TxOut>,
    ) -> Result<Transaction, Error> {
        let inputs = self.amount() + extra_inputs.iter().map(|u| u.value).sum();
        let mut output =
            vec![TxOut { value: next.amount(), script_pubkey: next.contract.script_pubkey() }];
        output.extend(extra_outputs);
        let outputs: Amount = output.iter().map(|o| o.value).sum();
        if outputs > inputs {
            return Err(Error::Amount(format!("the update pays {outputs} but only has {inputs}")));
        }
        Ok(Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: std::iter::once(outpoint)
                .chain(extra_inputs.iter().map(|u| u.outpoint))
                .map(|previous_output| TxIn { previous_output, ..Default::default() })
                .collect(),
            output,
        })
    }

    /// The previous outputs of an update transaction, as every signer needs them.
    pub fn update_prevouts(&self, extra_inputs: &[Utxo]) -> Vec<TxOut> {
        std::iter::once(TxOut {
            value: self.amount(),
            script_pubkey: self.contract.script_pubkey(),
        })
        .chain(
            extra_inputs
                .iter()
                .map(|u| TxOut { value: u.value, script_pubkey: u.script_pubkey.clone() }),
        )
        .collect()
    }

    /// One member's signature on an update.
    pub fn sign_update(
        &self,
        tx: &Transaction,
        prevouts: &[TxOut],
        key: &Keypair,
    ) -> Result<Signature, Error> {
        self.contract.multi_sign(COOP, tx, 0, prevouts, key)
    }

    /// Checks one member's signature on an update (the coordinator does this
    /// before accepting it).
    pub fn verify_update(
        &self,
        tx: &Transaction,
        prevouts: &[TxOut],
        pubkey: &XOnlyPublicKey,
        sig: &Signature,
    ) -> Result<(), Error> {
        let sighash = self.contract.multi_sighash(COOP, tx, 0, prevouts)?;
        crate::taproot::SECP
            .verify_schnorr(
                sig,
                &bitcoin::secp256k1::Message::from_digest(sighash.to_byte_array()),
                pubkey,
            )
            .map_err(|_| Error::WrongKey)
    }

    /// Puts everyone's signatures into the pool input.
    pub fn finalize_update(
        &self,
        tx: Transaction,
        sigs: &[(XOnlyPublicKey, Signature)],
    ) -> Result<Transaction, Error> {
        self.contract.multi_finalize(COOP, tx, 0, sigs)
    }
}

use bitcoin::hashes::Hash as _;

/// The pool shared by `inside` (indexes into `params.members`).
fn pool_contract(params: &PoolParams, inside: &[usize]) -> Contract {
    let members = |ids: &[usize]| ids.iter().map(|&i| &params.members[i]).collect::<Vec<_>>();
    let mut leaves = vec![Leaf::Multi {
        label: "Todos juntos: actualizar el pool".into(),
        signers: members(inside).iter().map(|m| m.pubkey).collect(),
    }];
    for &i in inside {
        let me = &params.members[i];
        let mine = Output::Pay {
            label: me.label.clone(),
            script_pubkey: me.payout.clone(),
            value: me.balance - params.fee,
        };
        let rest: Vec<usize> = inside.iter().copied().filter(|&m| m != i).collect();
        let outputs = if rest.len() == 1 {
            let other = &params.members[rest[0]];
            vec![
                mine,
                Output::Pay {
                    label: other.label.clone(),
                    script_pubkey: other.payout.clone(),
                    value: other.balance,
                },
            ]
        } else {
            let value = members(&rest).iter().map(|m| m.balance).sum();
            vec![Output::Contract { value, contract: pool_contract(params, &rest) }, mine]
        };
        leaves.push(Leaf::Ctv {
            label: format!("Sale {}", me.label),
            csv: None,
            signer: Some(me.pubkey),
            next: Template { label: format!("Salida de {}", me.label), outputs },
        });
    }
    Contract { label: format!("Pool de {}", inside.len()), leaves }
}

#[cfg(test)]
mod tests {
    use bitcoin::{Txid, hashes::Hash};

    use super::*;
    use crate::{
        taproot::SECP,
        wallet::{Role, Wallet},
    };

    fn key(n: u8) -> Keypair {
        Keypair::from_seckey_slice(&SECP, &[n; 32]).unwrap()
    }

    fn spk(n: u8) -> ScriptBuf {
        ScriptBuf::new_p2tr(&SECP, key(n).x_only_public_key().0, None)
    }

    fn params(balances: &[u64]) -> PoolParams {
        PoolParams {
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
        }
    }

    fn op() -> OutPoint {
        OutPoint { txid: Txid::all_zeros(), vout: 0 }
    }

    #[test]
    fn exits_pay_current_balances() {
        let p = Pool::new(params(&[100_000, 50_000, 30_000])).unwrap();
        assert_eq!(p.amount(), Amount::from_sat(180_000));
        let t1 = p.exit(&[], 1, op(), &key(2)).unwrap();
        assert_eq!(t1.output[0].value, Amount::from_sat(130_000));
        assert_eq!(t1.output[1], TxOut { value: Amount::from_sat(49_500), script_pubkey: spk(2) });
        let t2 = p.exit(&[1], 2, OutPoint { txid: t1.compute_txid(), vout: 0 }, &key(3)).unwrap();
        assert_eq!(t2.output[0].value, Amount::from_sat(29_500));
        assert_eq!(t2.output[1].value, Amount::from_sat(100_000));
    }

    #[test]
    fn nobody_can_push_someone_else_out() {
        let p = Pool::new(params(&[100_000, 100_000, 100_000])).unwrap();
        assert!(matches!(p.exit(&[], 1, op(), &key(1)), Err(Error::WrongKey)));
    }

    #[test]
    fn cooperative_payment_needs_everyone() {
        let p = Pool::new(params(&[100_000, 100_000, 100_000])).unwrap();
        // m1 pays m2 20.000 and the fee
        let next = Pool::new(params(&[79_500, 120_000, 100_000])).unwrap();
        let tx = p.update_tx(op(), &next, &[], vec![]).unwrap();
        assert_eq!(tx.output[0].value, Amount::from_sat(299_500));
        let prevouts = p.update_prevouts(&[]);
        let sigs: Vec<_> = (1..=3)
            .map(|n| {
                (key(n).x_only_public_key().0, p.sign_update(&tx, &prevouts, &key(n)).unwrap())
            })
            .collect();
        for (pk, sig) in &sigs {
            p.verify_update(&tx, &prevouts, pk, sig).unwrap();
        }
        assert!(p.finalize_update(tx.clone(), &sigs[..2]).is_err(), "two of three is not enough");
        let done = p.finalize_update(tx, &sigs).unwrap();
        assert_eq!(done.input[0].witness.len(), 3 + 2); // 3 signatures + script + control block
        assert!(p.sign_update(&done, &prevouts, &key(9)).is_err());
    }

    #[test]
    fn deposit_adds_an_input() {
        let p = Pool::new(params(&[100_000, 100_000])).unwrap();
        let w = Wallet::from_seed([7; 32]);
        let coin = Utxo {
            outpoint: op(),
            value: Amount::from_sat(50_000),
            script_pubkey: w.script_pubkey(Role::Main),
        };
        let next = Pool::new(params(&[149_500, 100_000])).unwrap();
        let tx = p.update_tx(op(), &next, std::slice::from_ref(&coin), vec![]).unwrap();
        assert_eq!(tx.input.len(), 2);
        let prevouts = p.update_prevouts(&[coin]);
        let witness = w.sign_inputs(&tx, &prevouts, &[1]).unwrap();
        assert_eq!(witness[0].len(), 1);
        assert!(w.sign_inputs(&tx, &prevouts, &[0]).is_err(), "the pool input isn't the wallet's");
    }

    #[test]
    fn six_members_compile() {
        let p = Pool::new(params(&[10_000; 6])).unwrap();
        assert_eq!(p.contract.leaves.len(), 7);
        assert!(Pool::new(params(&[10_000, 600])).is_err(), "a balance below fee + dust");
    }
}
