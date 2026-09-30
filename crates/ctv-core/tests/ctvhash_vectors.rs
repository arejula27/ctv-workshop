//! Official BIP-119 test vectors (bitcoin/bips, bip-0119/vectors/ctvhash.json, BSD-3-Clause).

use bitcoin::{Transaction, consensus::encode::deserialize_hex, hashes::Hash, hex::DisplayHex};
use ctv_core::ctv::template_hash;
use serde_json::Value;

#[test]
fn bip119_vectors() {
    let vectors: Vec<Value> =
        serde_json::from_str(include_str!("vectors/bip119-ctvhash.json")).unwrap();
    let mut checked = 0;
    for v in vectors.iter().filter(|v| v.is_object()) {
        let tx: Transaction = deserialize_hex(v["hex_tx"].as_str().unwrap()).unwrap();
        let indexes = v["spend_index"].as_array().unwrap();
        let results = v["result"].as_array().unwrap();
        for (idx, expected) in indexes.iter().zip(results) {
            let got = template_hash(&tx, idx.as_u64().unwrap() as u32);
            assert_eq!(
                got.to_byte_array().to_lower_hex_string(),
                expected.as_str().unwrap(),
                "vector: {}",
                v["desc"]
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} hashes checked");
}
