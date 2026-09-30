//! BIP-119 `OP_CHECKTEMPLATEVERIFY`: standard template hash and locking script.

use bitcoin::{
    Opcode, ScriptBuf, Transaction,
    consensus::Encodable,
    hashes::{Hash, sha256},
    opcodes::all::OP_NOP4,
    script::Builder,
};

/// CTV reuses `OP_NOP4` (0xb3).
pub const OP_CHECKTEMPLATEVERIFY: Opcode = OP_NOP4;

/// The DefaultCheckTemplateVerifyHash of BIP-119 for `tx` spent at `input_index`.
///
/// Commits to everything except the inputs' outpoints and witnesses, which is
/// what lets us precompute a whole tree of transactions before it is funded.
pub fn template_hash(tx: &Transaction, input_index: u32) -> sha256::Hash {
    let mut buf = Vec::with_capacity(32 * 3 + 4 * 5);
    enc(&tx.version, &mut buf);
    enc(&tx.lock_time, &mut buf);

    // scriptSigs are only committed to when at least one is non-empty
    if tx.input.iter().any(|i| !i.script_sig.is_empty()) {
        let mut sigs = Vec::new();
        for input in &tx.input {
            enc(&input.script_sig, &mut sigs);
        }
        buf.extend(sha256::Hash::hash(&sigs).to_byte_array());
    }

    buf.extend((tx.input.len() as u32).to_le_bytes());
    let mut seqs = Vec::with_capacity(4 * tx.input.len());
    for input in &tx.input {
        enc(&input.sequence, &mut seqs);
    }
    buf.extend(sha256::Hash::hash(&seqs).to_byte_array());

    buf.extend((tx.output.len() as u32).to_le_bytes());
    let mut outs = Vec::new();
    for output in &tx.output {
        enc(output, &mut outs);
    }
    buf.extend(sha256::Hash::hash(&outs).to_byte_array());

    buf.extend(input_index.to_le_bytes());
    sha256::Hash::hash(&buf)
}

/// `<hash> OP_CHECKTEMPLATEVERIFY`, the bare CTV tapscript.
pub fn ctv_script(hash: sha256::Hash) -> ScriptBuf {
    Builder::new()
        .push_slice(hash.to_byte_array())
        .push_opcode(OP_CHECKTEMPLATEVERIFY)
        .into_script()
}

fn enc<T: Encodable>(v: &T, buf: &mut Vec<u8>) {
    v.consensus_encode(buf).expect("writing to a Vec never fails");
}
