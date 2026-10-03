//! Generic tree of CTV templates.
//!
//! Every demo is the same shape: a taproot output (`Contract`) whose leaves are
//! either a CTV commitment to the next transaction (`Leaf::Ctv`) or a key escape
//! hatch (`Leaf::Key`). Trees are compiled leaves-first, because each CTV hash
//! commits to the output scripts of the transaction below it.

use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, TapLeafHash, TapSighashType,
    Transaction, TxIn, TxOut, Witness, XOnlyPublicKey,
    absolute::LockTime,
    hashes::{Hash, sha256},
    key::Keypair,
    opcodes::all::{OP_CHECKSIG, OP_CHECKSIGADD, OP_CHECKSIGVERIFY, OP_CSV, OP_DROP, OP_NUMEQUAL},
    script::Builder,
    secp256k1::Message,
    sighash::{Prevouts, SighashCache},
    taproot::{LeafVersion, TaprootSpendInfo},
    transaction::Version,
};
use serde::Serialize;

use crate::{
    Error,
    ctv::{push_template_check, template_hash},
    taproot::{self, SECP},
};

/// Sequence for leaves without a relative timelock (CTV commits to it too).
pub const NO_CSV: Sequence = Sequence::ENABLE_RBF_NO_LOCKTIME;

// ---------- description ----------

#[derive(Clone, Debug)]
pub struct Contract {
    pub label: String,
    pub leaves: Vec<Leaf>,
}

#[derive(Clone, Debug)]
pub enum Leaf {
    /// `[<csv> CSV DROP] [<signer> CHECKSIGVERIFY] TEMPLATEHASH <H(next)> EQUAL`: the coins can only
    /// move into `next`, optionally after `csv` blocks and/or with `signer`'s signature.
    Ctv { label: String, csv: Option<u16>, signer: Option<XOnlyPublicKey>, next: Template },
    /// `[<csv> CSV DROP] <signer> CHECKSIG`: `signer` spends anywhere.
    Key { label: String, csv: Option<u16>, signer: XOnlyPublicKey },
    /// `<pk1> CHECKSIG <pk2> CHECKSIGADD … <n> NUMEQUAL`: all `signers` together
    /// spend anywhere (a pool's cooperative update).
    Multi { label: String, signers: Vec<XOnlyPublicKey> },
}

/// A transaction fixed by CTV: version 2, locktime 0, one input.
#[derive(Clone, Debug)]
pub struct Template {
    pub label: String,
    pub outputs: Vec<Output>,
}

#[derive(Clone, Debug)]
pub enum Output {
    Pay { label: String, script_pubkey: ScriptBuf, value: Amount },
    Contract { value: Amount, contract: Contract },
}

impl Output {
    pub fn value(&self) -> Amount {
        match self {
            Output::Pay { value, .. } | Output::Contract { value, .. } => *value,
        }
    }
}

// ---------- compiled ----------

#[derive(Clone, Debug)]
pub struct CompiledContract {
    pub label: String,
    pub value: Amount,
    pub spend_info: TaprootSpendInfo,
    pub leaves: Vec<CompiledLeaf>,
}

#[derive(Clone, Debug)]
pub struct CompiledLeaf {
    pub label: String,
    pub script: ScriptBuf,
    pub csv: Option<u16>,
    pub signer: Option<XOnlyPublicKey>,
    /// All of them must sign (multisig leaves only), in script order.
    pub cosigners: Vec<XOnlyPublicKey>,
    /// The transaction this leaf is locked to; `None` for key leaves.
    pub template: Option<CompiledTemplate>,
}

#[derive(Clone, Debug)]
pub struct CompiledTemplate {
    pub label: String,
    /// Unsigned, with a null input outpoint to fill in when spending.
    pub tx: Transaction,
    pub ctv_hash: sha256::Hash,
    /// The contract behind each output, if any (same indexes as `tx.output`).
    pub children: Vec<Option<CompiledContract>>,
    pub output_labels: Vec<String>,
}

impl Contract {
    pub fn compile(&self, value: Amount) -> Result<CompiledContract, Error> {
        let mut leaves = Vec::with_capacity(self.leaves.len());
        for leaf in &self.leaves {
            leaves.push(match leaf {
                Leaf::Ctv { label, csv, signer, next } => {
                    let template = next.compile(sequence(*csv))?;
                    let out: Amount = template.tx.output.iter().map(|o| o.value).sum();
                    if out > value {
                        return Err(Error::Amount(format!(
                            "'{}' pays {out} but '{}' only holds {value}",
                            next.label, self.label
                        )));
                    }
                    CompiledLeaf {
                        label: label.clone(),
                        script: ctv_leaf_script(*csv, *signer, template.ctv_hash),
                        csv: *csv,
                        signer: *signer,
                        cosigners: vec![],
                        template: Some(template),
                    }
                }
                Leaf::Key { label, csv, signer } => CompiledLeaf {
                    label: label.clone(),
                    script: key_leaf_script(*csv, *signer),
                    csv: *csv,
                    signer: Some(*signer),
                    cosigners: vec![],
                    template: None,
                },
                Leaf::Multi { label, signers } => CompiledLeaf {
                    label: label.clone(),
                    script: multi_leaf_script(signers),
                    csv: None,
                    signer: None,
                    cosigners: signers.clone(),
                    template: None,
                },
            });
        }
        let scripts: Vec<_> = leaves.iter().map(|l| l.script.clone()).collect();
        Ok(CompiledContract {
            label: self.label.clone(),
            value,
            spend_info: taproot::build(&scripts)?,
            leaves,
        })
    }
}

impl Template {
    fn compile(&self, sequence: Sequence) -> Result<CompiledTemplate, Error> {
        let mut outputs = Vec::with_capacity(self.outputs.len());
        let mut children = Vec::with_capacity(self.outputs.len());
        let mut output_labels = Vec::with_capacity(self.outputs.len());
        for out in &self.outputs {
            match out {
                Output::Pay { label, script_pubkey, value } => {
                    outputs.push(TxOut { value: *value, script_pubkey: script_pubkey.clone() });
                    children.push(None);
                    output_labels.push(label.clone());
                }
                Output::Contract { value, contract } => {
                    let child = contract.compile(*value)?;
                    outputs.push(TxOut { value: *value, script_pubkey: child.script_pubkey() });
                    output_labels.push(contract.label.clone());
                    children.push(Some(child));
                }
            }
        }
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn { sequence, ..Default::default() }],
            output: outputs,
        };
        Ok(CompiledTemplate {
            label: self.label.clone(),
            ctv_hash: template_hash(&tx, 0),
            tx,
            children,
            output_labels,
        })
    }
}

impl CompiledTemplate {
    /// The template spending `outpoint`, still unsigned.
    pub fn bind(&self, outpoint: OutPoint) -> Transaction {
        let mut tx = self.tx.clone();
        tx.input[0].previous_output = outpoint;
        tx
    }

    /// Txid once the parent output is known. Witnesses don't affect it, so the
    /// whole tree's txids are known as soon as the root is funded.
    pub fn txid(&self, outpoint: OutPoint) -> bitcoin::Txid {
        self.bind(outpoint).compute_txid()
    }

    pub fn fee(&self, input_value: Amount) -> Amount {
        input_value - self.tx.output.iter().map(|o| o.value).sum()
    }
}

impl CompiledContract {
    pub fn script_pubkey(&self) -> ScriptBuf {
        ScriptBuf::new_p2tr_tweaked(self.spend_info.output_key())
    }

    pub fn address(&self, network: Network) -> Address {
        Address::p2tr_tweaked(self.spend_info.output_key(), network)
    }

    pub fn leaf(&self, index: usize) -> Result<&CompiledLeaf, Error> {
        self.leaves.get(index).ok_or(Error::Leaf(index))
    }

    /// Spends the CTV leaf `index` of this contract, funded at `outpoint`.
    /// `key` is required when the leaf has a signer.
    pub fn spend_ctv(
        &self,
        index: usize,
        outpoint: OutPoint,
        key: Option<&Keypair>,
    ) -> Result<Transaction, Error> {
        let leaf = self.leaf(index)?;
        let template = leaf.template.as_ref().ok_or(Error::NotCtv(index))?;
        let tx = template.bind(outpoint);
        self.finalize(leaf, tx, key)
    }

    /// Spends the key leaf `index` to arbitrary `outputs`.
    pub fn spend_key(
        &self,
        index: usize,
        outpoint: OutPoint,
        outputs: Vec<TxOut>,
        key: &Keypair,
    ) -> Result<Transaction, Error> {
        let leaf = self.leaf(index)?;
        if leaf.template.is_some() {
            return Err(Error::NotKey(index));
        }
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: outpoint,
                sequence: sequence(leaf.csv),
                ..Default::default()
            }],
            output: outputs,
        };
        self.finalize(leaf, tx, Some(key))
    }

    fn finalize(
        &self,
        leaf: &CompiledLeaf,
        mut tx: Transaction,
        key: Option<&Keypair>,
    ) -> Result<Transaction, Error> {
        let mut witness = Witness::new();
        if let Some(signer) = leaf.signer {
            let key = key.ok_or(Error::MissingKey)?;
            if key.x_only_public_key().0 != signer {
                return Err(Error::WrongKey);
            }
            witness.push(self.sign(leaf, &tx, key)?);
        }
        let control = self
            .spend_info
            .control_block(&(leaf.script.clone(), LeafVersion::TapScript))
            .expect("leaf belongs to this tree");
        witness.push(leaf.script.as_bytes());
        witness.push(control.serialize());
        tx.input[0].witness = witness;
        Ok(tx)
    }

    fn sign(&self, leaf: &CompiledLeaf, tx: &Transaction, key: &Keypair) -> Result<Vec<u8>, Error> {
        let prevout = TxOut { value: self.value, script_pubkey: self.script_pubkey() };
        let sighash = SighashCache::new(tx)
            .taproot_script_spend_signature_hash(
                0,
                &Prevouts::All(&[prevout]),
                TapLeafHash::from_script(&leaf.script, LeafVersion::TapScript),
                TapSighashType::Default,
            )
            .map_err(|e| Error::Sighash(e.to_string()))?;
        // Deterministic signing: no RNG needed, which keeps wasm builds simple.
        let sig =
            SECP.sign_schnorr_no_aux_rand(&Message::from_digest(sighash.to_byte_array()), key);
        Ok(bitcoin::taproot::Signature { signature: sig, sighash_type: TapSighashType::Default }
            .to_vec())
    }
}

fn sequence(csv: Option<u16>) -> Sequence {
    csv.map(Sequence::from_height).unwrap_or(NO_CSV)
}

fn push_csv(builder: Builder, csv: Option<u16>) -> Builder {
    match csv {
        Some(blocks) => builder.push_int(blocks as i64).push_opcode(OP_CSV).push_opcode(OP_DROP),
        None => builder,
    }
}

fn ctv_leaf_script(
    csv: Option<u16>,
    signer: Option<XOnlyPublicKey>,
    hash: sha256::Hash,
) -> ScriptBuf {
    let mut b = push_csv(Builder::new(), csv);
    if let Some(pk) = signer {
        b = b.push_x_only_key(&pk).push_opcode(OP_CHECKSIGVERIFY);
    }
    // EQUAL leaves 1 on a match: a single truthy element, as tapscript requires.
    push_template_check(b, hash).into_script()
}

fn multi_leaf_script(signers: &[XOnlyPublicKey]) -> ScriptBuf {
    let mut b = Builder::new();
    for (i, pk) in signers.iter().enumerate() {
        b = b.push_x_only_key(pk).push_opcode(if i == 0 { OP_CHECKSIG } else { OP_CHECKSIGADD });
    }
    b.push_int(signers.len() as i64).push_opcode(OP_NUMEQUAL).into_script()
}

fn key_leaf_script(csv: Option<u16>, signer: XOnlyPublicKey) -> ScriptBuf {
    push_csv(Builder::new(), csv).push_x_only_key(&signer).push_opcode(OP_CHECKSIG).into_script()
}

// ---------- view (for the tree visualizer) ----------

#[derive(Clone, Debug, Serialize)]
pub struct ContractView {
    pub label: String,
    pub value_sat: u64,
    pub address: String,
    /// Where it is funded, when known.
    pub outpoint: Option<String>,
    pub leaves: Vec<LeafView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LeafView {
    pub label: String,
    pub kind: &'static str,
    pub csv: Option<u16>,
    pub signer: Option<String>,
    pub tx: Option<TxView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TxView {
    pub label: String,
    pub txid: Option<String>,
    pub fee_sat: u64,
    pub outputs: Vec<OutputView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OutputView {
    pub label: String,
    pub value_sat: u64,
    pub contract: Option<ContractView>,
}

impl CompiledContract {
    pub fn view(&self, network: Network, outpoint: Option<OutPoint>) -> ContractView {
        ContractView {
            label: self.label.clone(),
            value_sat: self.value.to_sat(),
            address: self.address(network).to_string(),
            outpoint: outpoint.map(|o| o.to_string()),
            leaves: self
                .leaves
                .iter()
                .map(|leaf| LeafView {
                    label: leaf.label.clone(),
                    kind: if leaf.template.is_some() {
                        "ctv"
                    } else if leaf.cosigners.is_empty() {
                        "key"
                    } else {
                        "multi"
                    },
                    csv: leaf.csv,
                    signer: leaf.signer.map(|pk| pk.to_string()),
                    tx: leaf.template.as_ref().map(|t| t.view(network, self.value, outpoint)),
                })
                .collect(),
        }
    }
}

impl CompiledTemplate {
    fn view(&self, network: Network, input_value: Amount, outpoint: Option<OutPoint>) -> TxView {
        let txid = outpoint.map(|o| self.txid(o));
        TxView {
            label: self.label.clone(),
            txid: txid.map(|t| t.to_string()),
            fee_sat: self.fee(input_value).to_sat(),
            outputs: self
                .tx
                .output
                .iter()
                .zip(&self.children)
                .zip(&self.output_labels)
                .enumerate()
                .map(|(vout, ((out, child), label))| OutputView {
                    label: label.clone(),
                    value_sat: out.value.to_sat(),
                    contract: child.as_ref().map(|c| {
                        c.view(network, txid.map(|txid| OutPoint { txid, vout: vout as u32 }))
                    }),
                })
                .collect(),
        }
    }
}

// ---------- multisig (cooperative) spends ----------

impl CompiledContract {
    /// Sighash every cosigner of multisig leaf `index` signs, for input `input` of
    /// `tx`. `prevouts` are all of `tx`'s inputs, in order.
    pub fn multi_sighash(
        &self,
        index: usize,
        tx: &Transaction,
        input: usize,
        prevouts: &[TxOut],
    ) -> Result<bitcoin::TapSighash, Error> {
        let leaf = self.leaf(index)?;
        SighashCache::new(tx)
            .taproot_script_spend_signature_hash(
                input,
                &Prevouts::All(prevouts),
                TapLeafHash::from_script(&leaf.script, LeafVersion::TapScript),
                TapSighashType::Default,
            )
            .map_err(|e| Error::Sighash(e.to_string()))
    }

    /// One cosigner's signature over [`Self::multi_sighash`].
    pub fn multi_sign(
        &self,
        index: usize,
        tx: &Transaction,
        input: usize,
        prevouts: &[TxOut],
        key: &Keypair,
    ) -> Result<bitcoin::secp256k1::schnorr::Signature, Error> {
        let leaf = self.leaf(index)?;
        if !leaf.cosigners.contains(&key.x_only_public_key().0) {
            return Err(Error::WrongKey);
        }
        let sighash = self.multi_sighash(index, tx, input, prevouts)?;
        Ok(SECP.sign_schnorr_no_aux_rand(&Message::from_digest(sighash.to_byte_array()), key))
    }

    /// Puts the cosigners' signatures (any order) into input `input`'s witness.
    pub fn multi_finalize(
        &self,
        index: usize,
        mut tx: Transaction,
        input: usize,
        sigs: &[(XOnlyPublicKey, bitcoin::secp256k1::schnorr::Signature)],
    ) -> Result<Transaction, Error> {
        let leaf = self.leaf(index)?;
        let mut witness = Witness::new();
        // The first CHECKSIG pops the top of the stack, so the first signer's
        // signature goes last.
        for pk in leaf.cosigners.iter().rev() {
            let sig = sigs.iter().find(|(k, _)| k == pk).ok_or(Error::MissingKey)?.1;
            witness.push(sig.as_ref());
        }
        let control = self
            .spend_info
            .control_block(&(leaf.script.clone(), LeafVersion::TapScript))
            .expect("leaf belongs to this tree");
        witness.push(leaf.script.as_bytes());
        witness.push(control.serialize());
        tx.input[input].witness = witness;
        Ok(tx)
    }
}
