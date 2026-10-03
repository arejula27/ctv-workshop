//! `OP_TEMPLATEHASH` (Bitcoin Inquisition): template hash and locking script.
//!
//! Same idea as BIP-119 CTV, different mechanics: instead of `<hash> CTV`
//! checking the hash, `OP_TEMPLATEHASH` pushes the spending transaction's
//! template hash and the script compares it, `TEMPLATEHASH <hash> EQUAL`.
//! It only exists in tapscript, which is all we use.

use bitcoin::{
    Opcode, ScriptBuf, Transaction,
    consensus::Encodable,
    hashes::{Hash, sha256},
    opcodes::all::{OP_EQUAL, OP_RETURN_206},
    script::Builder,
};

/// `OP_TEMPLATEHASH` takes over `OP_SUCCESS206` (0xce).
pub const OP_TEMPLATEHASH: Opcode = OP_RETURN_206;

/// Inquisition's `GetTemplateHash` for `tx` spent at `input_index`, no annex:
/// `TaggedHash("TemplateHash", version || locktime || sha_sequences ||
/// sha_outputs || annex_present || input_index)`.
///
/// Commits to everything except the inputs' outpoints, scriptSigs and
/// witnesses, which is what lets us precompute a whole tree of transactions
/// before it is funded.
pub fn template_hash(tx: &Transaction, input_index: u32) -> sha256::Hash {
    let tag = sha256::Hash::hash(b"TemplateHash");
    let mut buf = Vec::with_capacity(32 * 4 + 4 * 3 + 1);
    buf.extend(tag.to_byte_array());
    buf.extend(tag.to_byte_array());
    enc(&tx.version, &mut buf);
    enc(&tx.lock_time, &mut buf);

    let mut seqs = Vec::with_capacity(4 * tx.input.len());
    for input in &tx.input {
        enc(&input.sequence, &mut seqs);
    }
    buf.extend(sha256::Hash::hash(&seqs).to_byte_array());

    let mut outs = Vec::new();
    for output in &tx.output {
        enc(output, &mut outs);
    }
    buf.extend(sha256::Hash::hash(&outs).to_byte_array());

    buf.push(0); // annex_present: our templates never carry an annex
    buf.extend(input_index.to_le_bytes());
    sha256::Hash::hash(&buf)
}

/// `OP_TEMPLATEHASH <hash> OP_EQUAL`, the bare template tapscript.
pub fn ctv_script(hash: sha256::Hash) -> ScriptBuf {
    push_template_check(Builder::new(), hash).into_script()
}

/// Appends `OP_TEMPLATEHASH <hash> OP_EQUAL`, leaving 1 on the stack on a match.
pub fn push_template_check(builder: Builder, hash: sha256::Hash) -> Builder {
    builder.push_opcode(OP_TEMPLATEHASH).push_slice(hash.to_byte_array()).push_opcode(OP_EQUAL)
}

fn enc<T: Encodable>(v: &T, buf: &mut Vec<u8>) {
    v.consensus_encode(buf).expect("writing to a Vec never fails");
}
