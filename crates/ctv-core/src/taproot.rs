//! Taproot helpers: unspendable internal key and balanced script trees.

use std::sync::LazyLock;

use bitcoin::{
    ScriptBuf, XOnlyPublicKey,
    key::Secp256k1,
    secp256k1::All,
    taproot::{TaprootBuilder, TaprootSpendInfo},
};

use crate::Error;

pub static SECP: LazyLock<Secp256k1<All>> = LazyLock::new(Secp256k1::new);

/// BIP-341's provably unspendable point H (the "NUMS" key). Using it as the
/// internal key disables the key path, so coins can only move through leaves.
pub fn nums_key() -> XOnlyPublicKey {
    const H: [u8; 32] = [
        0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a,
        0x5e, 0x07, 0x8a, 0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80,
        0x3a, 0xc0,
    ];
    XOnlyPublicKey::from_slice(&H).expect("H is a valid x-only key")
}

/// Builds a taproot tree with the leaves as balanced as possible, keyed by NUMS.
pub fn build(scripts: &[ScriptBuf]) -> Result<TaprootSpendInfo, Error> {
    let mut builder = TaprootBuilder::new();
    for (depth, script) in balanced_depths(scripts.len()).into_iter().zip(scripts) {
        builder =
            builder.add_leaf(depth, script.clone()).map_err(|e| Error::Taproot(e.to_string()))?;
    }
    builder
        .finalize(&SECP, nums_key())
        .map_err(|_| Error::Taproot("incomplete taproot tree".into()))
}

/// Leaf depths for a balanced tree, in the DFS order `TaprootBuilder` expects.
/// From stutxo/op_ctv_payment_pool (MIT).
fn balanced_depths(n: usize) -> Vec<u8> {
    if n == 0 {
        return vec![];
    }
    let height = n.next_power_of_two().trailing_zeros() as u8;
    let mut depths = vec![height; n];
    let excess = n.next_power_of_two() - n;
    for d in depths.iter_mut().rev().take(excess) {
        *d = height - 1;
    }
    depths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depths() {
        assert_eq!(balanced_depths(1), vec![0]);
        assert_eq!(balanced_depths(2), vec![1, 1]);
        assert_eq!(balanced_depths(3), vec![2, 2, 1]);
        assert_eq!(balanced_depths(5), vec![3, 3, 2, 2, 2]);
    }

    #[test]
    fn builds_any_size() {
        for n in 1..=12 {
            let scripts: Vec<_> =
                (0..n).map(|i| ScriptBuf::from_bytes(vec![0x51, i as u8])).collect();
            let info = build(&scripts).unwrap();
            assert_eq!(info.script_map().len(), n);
        }
    }
}
