//! Minimal participant wallet: three P2TR keys derived from one seed.
//!
//! Test coins only: the seed lives in the browser and keys are derived with a
//! plain tagged hash, no BIP-32.

use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, TapSighashType, Transaction, TxIn, TxOut,
    Witness,
    absolute::LockTime,
    hashes::{Hash, sha256},
    key::{Keypair, TapTweak},
    secp256k1::{Message, SecretKey},
    sighash::{Prevouts, SighashCache},
    transaction::Version,
};
use serde::{Deserialize, Serialize};

use crate::{Error, taproot::SECP};

/// Smallest standard P2TR output.
pub const DUST: Amount = Amount::from_sat(330);
/// Fee rate for wallet transactions (sat/vB). Regtest relays anything; this
/// just keeps transactions looking realistic.
const FEE_RATE: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Everyday address: faucet, change, hot withdrawals.
    Main,
    /// The vault's hot key (the one that gets "stolen" in the game).
    Hot,
    /// The vault's cold key.
    Cold,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Main, Role::Hot, Role::Cold];

    fn tag(self) -> &'static str {
        match self {
            Role::Main => "main",
            Role::Hot => "hot",
            Role::Cold => "cold",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Utxo {
    pub outpoint: OutPoint,
    pub value: Amount,
    pub script_pubkey: ScriptBuf,
}

pub struct Wallet {
    seed: [u8; 32],
}

impl Wallet {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self { seed }
    }

    pub fn keypair(&self, role: Role) -> Keypair {
        let mut counter = 0u32;
        loop {
            let mut data = Vec::with_capacity(64);
            data.extend(b"ctv-workshop/");
            data.extend(role.tag().as_bytes());
            data.extend(self.seed);
            data.extend(counter.to_le_bytes());
            // Out-of-range scalars are astronomically unlikely; loop anyway.
            if let Ok(sk) = SecretKey::from_slice(sha256::Hash::hash(&data).as_byte_array()) {
                return Keypair::from_secret_key(&SECP, &sk);
            }
            counter += 1;
        }
    }

    pub fn script_pubkey(&self, role: Role) -> ScriptBuf {
        ScriptBuf::new_p2tr(&SECP, self.keypair(role).x_only_public_key().0, None)
    }

    pub fn address(&self, role: Role, network: Network) -> Address {
        Address::p2tr(&SECP, self.keypair(role).x_only_public_key().0, None, network)
    }

    fn role_of(&self, spk: &ScriptBuf) -> Option<Role> {
        Role::ALL.into_iter().find(|r| &self.script_pubkey(*r) == spk)
    }

    /// Pays `amount` to `to` from any of our UTXOs, change back to `Role::Main`.
    pub fn send(
        &self,
        utxos: &[Utxo],
        to: ScriptBuf,
        amount: Amount,
    ) -> Result<Transaction, Error> {
        if amount < DUST {
            return Err(Error::Amount(format!("minimum is {DUST}")));
        }
        let mut ours: Vec<&Utxo> =
            utxos.iter().filter(|u| self.role_of(&u.script_pubkey).is_some()).collect();
        ours.sort_by_key(|u| std::cmp::Reverse(u.value));

        let mut selected = Vec::new();
        let mut total = Amount::ZERO;
        for utxo in ours {
            selected.push(utxo.clone());
            total += utxo.value;
            if total >= amount + estimate_fee(selected.len(), 2) {
                break;
            }
        }
        let fee_with_change = estimate_fee(selected.len(), 2);
        if total < amount + estimate_fee(selected.len(), 1) {
            return Err(Error::Amount(format!(
                "not enough funds: have {total}, need {}",
                amount + estimate_fee(selected.len(), 1)
            )));
        }

        let mut output = vec![TxOut { value: amount, script_pubkey: to }];
        if let Some(change) = total.checked_sub(amount + fee_with_change).filter(|c| *c >= DUST) {
            output.push(TxOut { value: change, script_pubkey: self.script_pubkey(Role::Main) });
        }
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: selected
                .iter()
                .map(|u| TxIn { previous_output: u.outpoint, ..Default::default() })
                .collect(),
            output,
        };
        self.sign(tx, &selected)
    }

    /// Key-path signs every input; `prevouts` in input order.
    fn sign(&self, mut tx: Transaction, prevouts: &[Utxo]) -> Result<Transaction, Error> {
        let txouts: Vec<TxOut> = prevouts
            .iter()
            .map(|u| TxOut { value: u.value, script_pubkey: u.script_pubkey.clone() })
            .collect();
        let mut witnesses = Vec::with_capacity(prevouts.len());
        {
            let mut cache = SighashCache::new(&tx);
            for (i, utxo) in prevouts.iter().enumerate() {
                let role = self.role_of(&utxo.script_pubkey).expect("only our utxos are selected");
                let tweaked = self.keypair(role).tap_tweak(&SECP, None).to_keypair();
                let sighash = cache
                    .taproot_key_spend_signature_hash(
                        i,
                        &Prevouts::All(&txouts),
                        TapSighashType::Default,
                    )
                    .map_err(|e| Error::Sighash(e.to_string()))?;
                let sig = SECP.sign_schnorr_no_aux_rand(
                    &Message::from_digest(sighash.to_byte_array()),
                    &tweaked,
                );
                witnesses.push(Witness::from_slice(&[sig.as_ref()]));
            }
        }
        for (input, witness) in tx.input.iter_mut().zip(witnesses) {
            input.witness = witness;
        }
        Ok(tx)
    }
}

/// vsize of a P2TR key-path transaction, at `FEE_RATE`.
fn estimate_fee(inputs: usize, outputs: usize) -> Amount {
    // 10.5 vB overhead, 57.5 vB per key-path input, 43 vB per P2TR output
    let vbytes = (21 + 115 * inputs as u64 + 86 * outputs as u64).div_ceil(2);
    Amount::from_sat(vbytes * FEE_RATE)
}

#[cfg(test)]
mod tests {
    use bitcoin::{Txid, hashes::Hash};

    use super::*;

    fn wallet() -> Wallet {
        Wallet::from_seed([7; 32])
    }

    fn utxo(w: &Wallet, role: Role, sats: u64, n: u8) -> Utxo {
        Utxo {
            outpoint: OutPoint { txid: Txid::from_byte_array([n; 32]), vout: 0 },
            value: Amount::from_sat(sats),
            script_pubkey: w.script_pubkey(role),
        }
    }

    #[test]
    fn roles_have_distinct_keys() {
        let w = wallet();
        let pks: Vec<_> = Role::ALL.iter().map(|r| w.keypair(*r).x_only_public_key().0).collect();
        assert_ne!(pks[0], pks[1]);
        assert_ne!(pks[1], pks[2]);
        assert_eq!(
            w.keypair(Role::Hot).public_key(),
            Wallet::from_seed([7; 32]).keypair(Role::Hot).public_key()
        );
    }

    #[test]
    fn send_with_change() {
        let w = wallet();
        let to = Wallet::from_seed([8; 32]).script_pubkey(Role::Main);
        let tx = w
            .send(
                &[utxo(&w, Role::Main, 50_000, 1), utxo(&w, Role::Cold, 10_000, 2)],
                to.clone(),
                Amount::from_sat(30_000),
            )
            .unwrap();
        // the biggest UTXO is enough on its own
        assert_eq!(tx.input.len(), 1);
        assert_eq!(tx.output[0], TxOut { value: Amount::from_sat(30_000), script_pubkey: to });
        assert_eq!(tx.output[1].script_pubkey, w.script_pubkey(Role::Main));
        let fee = Amount::from_sat(50_000) - tx.output.iter().map(|o| o.value).sum();
        assert_eq!(fee, estimate_fee(1, 2));
        assert_eq!(tx.input[0].witness.len(), 1);
    }

    #[test]
    fn ignores_foreign_utxos_and_reports_shortfall() {
        let w = wallet();
        let other = Wallet::from_seed([9; 32]);
        let err = w
            .send(
                &[utxo(&other, Role::Main, 1_000_000, 1)],
                w.script_pubkey(Role::Hot),
                Amount::from_sat(1_000),
            )
            .unwrap_err();
        assert!(matches!(err, Error::Amount(_)));
    }
}
