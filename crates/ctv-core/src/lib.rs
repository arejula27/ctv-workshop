//! CTV contract core: templates, CTV hashes, taproot trees and spends.
//!
//! No I/O, so it compiles both natively and to `wasm32-unknown-unknown`.

pub mod contracts;
pub mod ctv;
pub mod taproot;
pub mod tree;
pub mod wallet;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("taproot: {0}")]
    Taproot(String),
    #[error("amounts: {0}")]
    Amount(String),
    #[error("no leaf {0}")]
    Leaf(usize),
    #[error("leaf {0} is not a CTV leaf")]
    NotCtv(usize),
    #[error("leaf {0} is not a key leaf")]
    NotKey(usize),
    #[error("this leaf needs a signature")]
    MissingKey,
    #[error("the key does not match the leaf's signer")]
    WrongKey,
    #[error("sighash: {0}")]
    Sighash(String),
}
