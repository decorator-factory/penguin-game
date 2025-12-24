#[cfg(target_family = "wasm")]
pub fn performance_timer() -> f64 {
    crate::wasm::penguin_monotonic_millis() / 1000.0
}

#[cfg(not(target_family = "wasm"))]
pub fn performance_timer() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
