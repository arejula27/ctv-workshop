//! wasm-bindgen bindings over `ctv-core` for the webapp.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
