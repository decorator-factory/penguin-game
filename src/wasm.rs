#[cfg(target_family = "wasm")]
unsafe extern "C" {
    // SAFETY: I double checked that the site defines importObject.url_has_demo_hash
    pub safe fn is_wasm_demo() -> bool;
}
