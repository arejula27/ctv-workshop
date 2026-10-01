//! Cancelable subscription: `count` payments of `payment` every `every` blocks.
//!
//! ```text
//! step k: [ <every> CSV DROP <H(pay_k)> CTV ]   pay_k: `payment` → receiver, rest → step k+1
//!         [ <payer> CHECKSIG ]                  cancel: the payer takes back what's left
//! ```
//!
//! Built from the last payment backwards, since each step commits to the next.
//! Step k holds `(count - k + 1) * (payment + fee)`, so every transaction pays
//! exactly one `fee`. Anyone can push a due payment (usually the receiver).

use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Transaction, TxOut, XOnlyPublicKey, key::Keypair,
};

use crate::{
    Error,
    tree::{CompiledContract, Contract, Leaf, Output, Template},
    wallet::DUST,
};

pub const PAY: usize = 0;
pub const CANCEL: usize = 1;
const MAX_PAYMENTS: u16 = 24;

#[derive(Clone, Debug)]
pub struct SubscriptionParams {
    pub payment: Amount,
    pub count: u16,
    /// Blocks between payments (the first one too).
    pub every: u16,
    pub receiver: ScriptBuf,
    pub payer: XOnlyPublicKey,
    pub fee: Amount,
}

#[derive(Clone, Debug)]
pub struct Subscription {
    pub params: SubscriptionParams,
    /// Step 1, the output that gets funded.
    pub contract: CompiledContract,
}

impl Subscription {
    pub fn new(params: SubscriptionParams) -> Result<Self, Error> {
        if params.count == 0 || params.count > MAX_PAYMENTS {
            return Err(Error::Amount(format!("1..={MAX_PAYMENTS} payments")));
        }
        if params.every == 0 {
            return Err(Error::Amount("at least 1 block between payments".into()));
        }
        if params.payment < DUST {
            return Err(Error::Amount(format!("each payment must be at least {DUST}")));
        }
        let mut next: Option<Contract> = None;
        for k in (1..=params.count).rev() {
            let mut outputs = vec![Output::Pay {
                label: "Cobro".into(),
                script_pubkey: params.receiver.clone(),
                value: params.payment,
            }];
            if let Some(rest) = next.take() {
                outputs.push(Output::Contract { value: params.step_value(k + 1), contract: rest });
            }
            next = Some(Contract {
                label: format!("Suscripción · quedan {}", params.count - k + 1),
                leaves: vec![
                    Leaf::Ctv {
                        label: format!("Pago {k} de {}", params.count),
                        csv: Some(params.every),
                        signer: None,
                        next: Template { label: format!("Pago {k}"), outputs },
                    },
                    Leaf::Key { label: "Cancelar".into(), csv: None, signer: params.payer },
                ],
            });
        }
        let contract = next.expect("count >= 1").compile(params.step_value(1))?;
        Ok(Self { params, contract })
    }

    pub fn amount(&self) -> Amount {
        self.params.step_value(1)
    }

    pub fn address(&self, network: Network) -> bitcoin::Address {
        self.contract.address(network)
    }

    /// The contract of step `k` (1-based).
    pub fn step(&self, k: u16) -> Result<&CompiledContract, Error> {
        let mut c = &self.contract;
        for _ in 1..k {
            c = c.leaves[PAY]
                .template
                .as_ref()
                .and_then(|t| t.children.get(1)?.as_ref())
                .ok_or(Error::Amount(format!("there is no step {k}")))?;
        }
        Ok(c)
    }

    /// Where step `k` sits, given where step 1 was funded.
    pub fn step_outpoint(&self, k: u16, funding: OutPoint) -> Result<OutPoint, Error> {
        let mut op = funding;
        for i in 1..k {
            let pay = self.step(i)?.leaves[PAY].template.as_ref().expect("pay leaf is CTV");
            op = OutPoint { txid: pay.txid(op), vout: 1 };
        }
        Ok(op)
    }

    /// Payment `k`, valid once step `k` has `every` confirmations.
    pub fn pay(&self, k: u16, funding: OutPoint) -> Result<Transaction, Error> {
        self.step(k)?.spend_ctv(PAY, self.step_outpoint(k, funding)?, None)
    }

    /// Cancels while at step `k`: the payer gets the rest back at `to`.
    pub fn cancel(
        &self,
        k: u16,
        funding: OutPoint,
        to: ScriptBuf,
        payer: &Keypair,
    ) -> Result<Transaction, Error> {
        let step = self.step(k)?;
        let out = TxOut { value: step.value - self.params.fee, script_pubkey: to };
        step.spend_key(CANCEL, self.step_outpoint(k, funding)?, vec![out], payer)
    }
}

impl SubscriptionParams {
    fn step_value(&self, k: u16) -> Amount {
        (self.payment + self.fee) * (self.count - k + 1) as u64
    }
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

    fn sub() -> Subscription {
        Subscription::new(SubscriptionParams {
            payment: Amount::from_sat(10_000),
            count: 3,
            every: 6,
            receiver: spk(2),
            payer: key(1).x_only_public_key().0,
            fee: Amount::from_sat(500),
        })
        .unwrap()
    }

    fn op() -> OutPoint {
        OutPoint { txid: Txid::all_zeros(), vout: 0 }
    }

    #[test]
    fn amounts_chain_exactly() {
        let s = sub();
        assert_eq!(s.amount(), Amount::from_sat(31_500));
        let p1 = s.pay(1, op()).unwrap();
        assert_eq!(p1.output[0], TxOut { value: Amount::from_sat(10_000), script_pubkey: spk(2) });
        assert_eq!(p1.output[1].value, Amount::from_sat(21_000));
        assert_eq!(p1.output[1].script_pubkey, s.step(2).unwrap().script_pubkey());
        assert_eq!(p1.input[0].sequence, Sequence::from_height(6));

        let p2 = s.pay(2, op()).unwrap();
        assert_eq!(p2.input[0].previous_output, OutPoint { txid: p1.compute_txid(), vout: 1 });
        let p3 = s.pay(3, op()).unwrap();
        assert_eq!(p3.output.len(), 1, "last payment has no change");
        assert_eq!(p3.input[0].previous_output.txid, p2.compute_txid());
        assert!(s.step(4).is_err());
    }

    #[test]
    fn cancel_returns_the_rest() {
        let s = sub();
        let tx = s.cancel(2, op(), spk(1), &key(1)).unwrap();
        assert_eq!(tx.output[0].value, Amount::from_sat(20_500));
        assert!(matches!(s.cancel(2, op(), spk(1), &key(2)), Err(Error::WrongKey)));
    }
}
