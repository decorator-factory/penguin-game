#[cfg(target_family = "wasm")]
unsafe extern "C" {
    // SAFETY: I double checked that the site defines importObject.url_has_demo_hash
    pub safe fn is_wasm_demo() -> bool;

    // SAFETY: I double checked that the site defines importObject.is_wasm_new_level
    pub safe fn is_wasm_new_level() -> bool;

    // SAFETY: I double checked that the site defines importObject.is_wasm_new_level_demo
    pub safe fn is_wasm_new_level_demo() -> bool;

    /// When we're panicking and are about to "die", provide a panic message that will
    /// be shown to the user. This should include things like the panic message and the line number.
    ///
    /// Safety: `message` must point at a 0-terminated UTF-8 string
    pub unsafe fn set_panic_message(begin: *const core::ffi::c_char);
}
