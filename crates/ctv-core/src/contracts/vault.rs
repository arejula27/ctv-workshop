//! Single-hop vault (design from jamesob/simple-ctv-vault, rewritten for taproot).
//!
//! ```text
//! vault:   [ CTV → start withdrawal ]            | [ CTV → cold key ]
//! unvault: [ <delay> CSV DROP <hot> CHECKSIG ]   | [ CTV → cold key ]   ("panic button")
//! ```
//!
//! Starting a withdrawal and pressing panic need no signature: anyone, even a
//! watchtower, can do it. A thief holding the hot key still has to wait `delay`
//! blocks, and during that time the owner can sweep everything to the cold key.

use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Transaction, TxOut, XOnlyPublicKey, key::Keypair,
};

use crate::{
    Error,
    taproot::SECP,
    tree::{CompiledContract, CompiledTemplate, Contract, Leaf, Output, Template},
};

/// Leaf indexes of the vault output.
pub const VAULT_START: usize = 0;
pub const VAULT_TO_COLD: usize = 1;
/// Leaf indexes of the unvault (withdrawal in progress) output.
pub const UNVAULT_HOT: usize = 0;
pub const UNVAULT_PANIC: usize = 1;

/// Smallest standard P2TR output.
const DUST: Amount = Amount::from_sat(330);

#[derive(Clone, Debug)]
pub struct VaultParams {
    /// Value of the vault output.
    pub amount: Amount,
    pub hot: XOnlyPublicKey,
    pub cold: XOnlyPublicKey,
    /// Blocks the hot key must wait after a withdrawal starts.
    pub delay: u16,
    /// Fixed fee paid by each transaction of the tree.
    pub fee: Amount,
}

#[derive(Clone, Debug)]
pub struct Vault {
    pub params: VaultParams,
    pub contract: CompiledContract,
}

impl Vault {
    pub fn new(params: VaultParams) -> Result<Self, Error> {
        let VaultParams { amount, hot, cold, delay, fee } = params.clone();
        // Worst path: vault → unvault → hot/cold pays two fees.
        if amount < fee * 2 + DUST {
            return Err(Error::Amount(format!("a vault needs at least {}", fee * 2 + DUST)));
        }
        if delay == 0 {
            return Err(Error::Amount("delay must be at least 1 block".into()));
        }
        let cold_spk = ScriptBuf::new_p2tr(&SECP, cold, None);
        let to_cold = |value: Amount| Template {
            label: "A la clave fría".into(),
            outputs: vec![Output::Pay {
                label: "Clave fría".into(),
                script_pubkey: cold_spk.clone(),
                value,
            }],
        };

        let unvault_value = amount - fee;
        let unvault = Contract {
            label: "Retiro en curso".into(),
            leaves: vec![
                Leaf::Key {
                    label: format!("Clave hot, tras {delay} bloques"),
                    csv: Some(delay),
                    signer: hot,
                },
                Leaf::Ctv {
                    label: "Botón del pánico".into(),
                    csv: None,
                    signer: None,
                    next: to_cold(unvault_value - fee),
                },
            ],
        };
        let vault = Contract {
            label: "Vault".into(),
            leaves: vec![
                Leaf::Ctv {
                    label: "Iniciar retiro".into(),
                    csv: None,
                    signer: None,
                    next: Template {
                        label: "Inicio del retiro".into(),
                        outputs: vec![Output::Contract { value: unvault_value, contract: unvault }],
                    },
                },
                Leaf::Ctv {
                    label: "Mandar a la clave fría".into(),
                    csv: None,
                    signer: None,
                    next: to_cold(amount - fee),
                },
            ],
        };
        Ok(Self { contract: vault.compile(amount)?, params })
    }

    pub fn address(&self, network: Network) -> bitcoin::Address {
        self.contract.address(network)
    }

    fn start_template(&self) -> &CompiledTemplate {
        self.contract.leaves[VAULT_START].template.as_ref().expect("start leaf is CTV")
    }

    /// The "withdrawal in progress" contract created by [`Self::start_withdrawal`].
    pub fn unvault(&self) -> &CompiledContract {
        self.start_template().children[0].as_ref().expect("start pays to the unvault contract")
    }

    /// Where the unvault output lands for a vault funded at `vault_outpoint`.
    pub fn unvault_outpoint(&self, vault_outpoint: OutPoint) -> OutPoint {
        OutPoint { txid: self.start_template().txid(vault_outpoint), vout: 0 }
    }

    pub fn start_withdrawal(&self, vault_outpoint: OutPoint) -> Result<Transaction, Error> {
        self.contract.spend_ctv(VAULT_START, vault_outpoint, None)
    }

    pub fn to_cold(&self, vault_outpoint: OutPoint) -> Result<Transaction, Error> {
        self.contract.spend_ctv(VAULT_TO_COLD, vault_outpoint, None)
    }

    pub fn panic(&self, unvault_outpoint: OutPoint) -> Result<Transaction, Error> {
        self.unvault().spend_ctv(UNVAULT_PANIC, unvault_outpoint, None)
    }

    /// Hot-key withdrawal of everything left (minus the fee) to `to`. Only valid
    /// on chain once the unvault output has `delay` confirmations.
    pub fn withdraw_hot(
        &self,
        unvault_outpoint: OutPoint,
        to: ScriptBuf,
        hot: &Keypair,
    ) -> Result<Transaction, Error> {
        let value = self.unvault().value - self.params.fee;
        self.unvault().spend_key(
            UNVAULT_HOT,
            unvault_outpoint,
            vec![TxOut { value, script_pubkey: to }],
            hot,
        )
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::{Sequence, Txid, hashes::Hash};

    use super::*;

    fn key(n: u8) -> Keypair {
        Keypair::from_seckey_slice(&SECP, &[n; 32]).unwrap()
    }

    fn vault() -> Vault {
        Vault::new(VaultParams {
            amount: Amount::from_sat(100_000),
            hot: key(1).x_only_public_key().0,
            cold: key(2).x_only_public_key().0,
            delay: 10,
            fee: Amount::from_sat(500),
        })
        .unwrap()
    }

    fn outpoint() -> OutPoint {
        OutPoint { txid: Txid::all_zeros(), vout: 0 }
    }

    #[test]
    fn deterministic_address() {
        assert_eq!(vault().address(Network::Regtest), vault().address(Network::Regtest));
    }

    #[test]
    fn amounts_along_every_path() {
        let v = vault();
        let op = outpoint();
        let cold = ScriptBuf::new_p2tr(&SECP, v.params.cold, None);

        let start = v.start_withdrawal(op).unwrap();
        assert_eq!(start.output[0].value, Amount::from_sat(99_500));
        assert_eq!(start.output[0].script_pubkey, v.unvault().script_pubkey());

        let to_cold = v.to_cold(op).unwrap();
        assert_eq!(
            to_cold.output,
            vec![TxOut { value: Amount::from_sat(99_500), script_pubkey: cold.clone() }]
        );

        let uop = v.unvault_outpoint(op);
        assert_eq!(uop.txid, start.compute_txid());
        let panic = v.panic(uop).unwrap();
        assert_eq!(
            panic.output,
            vec![TxOut { value: Amount::from_sat(99_000), script_pubkey: cold }]
        );

        let hot = v.withdraw_hot(uop, ScriptBuf::new(), &key(1)).unwrap();
        assert_eq!(hot.output[0].value, Amount::from_sat(99_000));
        assert_eq!(hot.input[0].sequence, Sequence::from_height(10));
        // signature + script + control block
        assert_eq!(hot.input[0].witness.len(), 3);
        assert_eq!(panic.input[0].witness.len(), 2);
    }

    #[test]
    fn hot_withdrawal_needs_the_hot_key() {
        let v = vault();
        let uop = v.unvault_outpoint(outpoint());
        assert!(matches!(v.withdraw_hot(uop, ScriptBuf::new(), &key(9)), Err(Error::WrongKey)));
    }

    #[test]
    fn rejects_tiny_vaults() {
        let mut p = vault().params;
        p.amount = Amount::from_sat(1_000);
        assert!(Vault::new(p).is_err());
    }

    #[test]
    fn view_has_txids_once_funded() {
        let v = vault();
        let view = v.contract.view(Network::Regtest, Some(outpoint()));
        let start = view.leaves[VAULT_START].tx.as_ref().unwrap();
        assert_eq!(
            start.txid.as_deref(),
            Some(v.unvault_outpoint(outpoint()).txid.to_string().as_str())
        );
        let unvault = start.outputs[0].contract.as_ref().unwrap();
        assert_eq!(unvault.leaves[UNVAULT_HOT].kind, "key");
        assert_eq!(unvault.leaves[UNVAULT_HOT].csv, Some(10));
        assert!(unvault.outpoint.is_some());
    }
}
