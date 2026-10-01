//! Inheritance: if the owner doesn't move the coins for `delay` blocks, anyone
//! can execute a split fixed in advance.
//!
//! ```text
//! inheritance: [ <owner> CHECKSIG ]                   owner moves it: "I'm alive"
//!              [ <delay> CSV DROP <H(split)> CTV ]    after `delay` idle blocks: split to heirs
//! ```
//!
//! "I'm alive" spends the owner leaf into a fresh inheritance with the same
//! heirs, which restarts the relative timelock. Shares are proportional, so the
//! new (fee-reduced) amount is split the same way.

use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Transaction, TxOut, XOnlyPublicKey, key::Keypair,
};

use crate::{
    Error,
    tree::{CompiledContract, Contract, Leaf, Output, Template},
    wallet::DUST,
};

pub const OWNER: usize = 0;
pub const SPLIT: usize = 1;

#[derive(Clone, Debug)]
pub struct Heir {
    pub label: String,
    pub script_pubkey: ScriptBuf,
    /// Relative weight; heirs get `amount * share / Σ shares`.
    pub share: u32,
}

#[derive(Clone, Debug)]
pub struct InheritanceParams {
    pub amount: Amount,
    pub owner: XOnlyPublicKey,
    pub heirs: Vec<Heir>,
    /// Idle blocks before the split can be executed.
    pub delay: u16,
    pub fee: Amount,
}

#[derive(Clone, Debug)]
pub struct Inheritance {
    pub params: InheritanceParams,
    pub contract: CompiledContract,
}

impl Inheritance {
    pub fn new(params: InheritanceParams) -> Result<Self, Error> {
        if params.heirs.is_empty() {
            return Err(Error::Amount("at least one heir".into()));
        }
        if params.delay == 0 {
            return Err(Error::Amount("delay must be at least 1 block".into()));
        }
        let split = split_amounts(&params)?;
        let contract = Contract {
            label: "Herencia".into(),
            leaves: vec![
                Leaf::Key {
                    label: "La dueña la mueve (sigue viva)".into(),
                    csv: None,
                    signer: params.owner,
                },
                Leaf::Ctv {
                    label: format!("Reparto, tras {} bloques sin moverse", params.delay),
                    csv: Some(params.delay),
                    signer: None,
                    next: Template {
                        label: "Reparto".into(),
                        outputs: params
                            .heirs
                            .iter()
                            .zip(split)
                            .map(|(h, value)| Output::Pay {
                                label: h.label.clone(),
                                script_pubkey: h.script_pubkey.clone(),
                                value,
                            })
                            .collect(),
                    },
                },
            ],
        };
        Ok(Self { contract: contract.compile(params.amount)?, params })
    }

    pub fn address(&self, network: Network) -> bitcoin::Address {
        self.contract.address(network)
    }

    /// The same inheritance one "I'm alive" later (amount minus one fee).
    pub fn renewed(&self) -> Result<Self, Error> {
        let mut p = self.params.clone();
        p.amount = p.amount.checked_sub(p.fee).ok_or(Error::Amount("nothing left".into()))?;
        Self::new(p)
    }

    /// "I'm alive": moves the coins into [`Self::renewed`], restarting the clock.
    /// The new inheritance sits at output 0 of the returned transaction.
    pub fn keep_alive(
        &self,
        outpoint: OutPoint,
        owner: &Keypair,
    ) -> Result<(Transaction, Self), Error> {
        let next = self.renewed()?;
        let out = TxOut { value: next.params.amount, script_pubkey: next.contract.script_pubkey() };
        let tx = self.contract.spend_key(OWNER, outpoint, vec![out], owner)?;
        Ok((tx, next))
    }

    /// The owner takes everything back to `to`.
    pub fn withdraw(
        &self,
        outpoint: OutPoint,
        to: ScriptBuf,
        owner: &Keypair,
    ) -> Result<Transaction, Error> {
        let out = TxOut { value: self.params.amount - self.params.fee, script_pubkey: to };
        self.contract.spend_key(OWNER, outpoint, vec![out], owner)
    }

    /// Executes the split. Valid once the output has `delay` confirmations.
    pub fn claim(&self, outpoint: OutPoint) -> Result<Transaction, Error> {
        self.contract.spend_ctv(SPLIT, outpoint, None)
    }
}

/// Proportional split of `amount - fee`; rounding dust goes to the first heir.
fn split_amounts(p: &InheritanceParams) -> Result<Vec<Amount>, Error> {
    let total_shares: u64 = p.heirs.iter().map(|h| h.share as u64).sum();
    if total_shares == 0 {
        return Err(Error::Amount("shares must add up to more than 0".into()));
    }
    let pot = p.amount.checked_sub(p.fee).ok_or(Error::Amount("amount below the fee".into()))?;
    let mut amounts: Vec<Amount> = p
        .heirs
        .iter()
        .map(|h| Amount::from_sat(pot.to_sat() * h.share as u64 / total_shares))
        .collect();
    let assigned: Amount = amounts.iter().copied().sum();
    amounts[0] += pot - assigned;
    if amounts.iter().any(|a| *a < DUST) {
        return Err(Error::Amount(format!("every heir must get at least {DUST}")));
    }
    Ok(amounts)
}

#[cfg(test)]
mod tests {
    use bitcoin::{Sequence, Txid, hashes::Hash};

    use super::*;
    use crate::taproot::SECP;

    fn key(n: u8) -> Keypair {
        Keypair::from_seckey_slice(&SECP, &[n; 32]).unwrap()
    }

    fn spk(n: u8) -> ScriptBuf {
        ScriptBuf::new_p2tr(&SECP, key(n).x_only_public_key().0, None)
    }

    fn inheritance() -> Inheritance {
        Inheritance::new(InheritanceParams {
            amount: Amount::from_sat(100_000),
            owner: key(1).x_only_public_key().0,
            heirs: vec![
                Heir { label: "Hija".into(), script_pubkey: spk(2), share: 2 },
                Heir { label: "Hijo".into(), script_pubkey: spk(3), share: 1 },
            ],
            delay: 10,
            fee: Amount::from_sat(1_000),
        })
        .unwrap()
    }

    fn op() -> OutPoint {
        OutPoint { txid: Txid::all_zeros(), vout: 0 }
    }

    #[test]
    fn split_is_proportional_and_exact() {
        let tx = inheritance().claim(op()).unwrap();
        // 99_000 split 2:1, rounding dust to the first heir
        assert_eq!(tx.output[0].value, Amount::from_sat(66_000));
        assert_eq!(tx.output[1].value, Amount::from_sat(33_000));
        assert_eq!(tx.input[0].sequence, Sequence::from_height(10));
    }

    #[test]
    fn keep_alive_renews_with_same_heirs() {
        let i = inheritance();
        let (tx, next) = i.keep_alive(op(), &key(1)).unwrap();
        assert_eq!(tx.output[0].script_pubkey, next.contract.script_pubkey());
        assert_eq!(next.params.amount, Amount::from_sat(99_000));
        assert_ne!(next.contract.script_pubkey(), i.contract.script_pubkey());
        let split = next.claim(op()).unwrap();
        assert_eq!(split.output[0].value + split.output[1].value, Amount::from_sat(98_000));
    }

    #[test]
    fn only_the_owner_keeps_it_alive() {
        assert!(matches!(inheritance().keep_alive(op(), &key(9)), Err(Error::WrongKey)));
    }
}
