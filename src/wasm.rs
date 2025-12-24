use std::{
    collections::HashMap,
    mem::MaybeUninit,
};

unsafe extern "C" {
    /// When we're panicking and are about to "die", provide a panic message that will
    /// be shown to the user. This should include things like the panic message and the line number.
    ///
    /// Safety: `message` must point at a 0-terminated UTF-8 string
    pub unsafe fn penguin_set_panic_message(begin: *const u8);

    unsafe fn penguin_read_options(begin: *mut u8, max_length: usize) -> usize;
}

pub fn read_options() -> HashMap<String, String> {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);

    // Safety:
    // - just allocated `capacity` bytes;
    // - `penguin_read_options` promises to return how many bytes it initialized;
    unsafe {
        let length = penguin_read_options(buf.as_mut_ptr(), buf.capacity());
        buf.set_len(length);
    }

    let mut rv = HashMap::with_capacity(16);

    for line in buf.split(|c| *c == 0) {
        let Some(index) = line.iter().position(|c| *c == 1) else {
            macroquad::logging::warn!("Invalid option line encountered");
            continue;
        };
        let Ok(key) = String::from_utf8(line[..index].to_vec()) else {
            macroquad::logging::warn!("Option with invalid key encountered");
            continue;
        };
        let Ok(value) = String::from_utf8(line[index + 1..].to_vec()) else {
            macroquad::logging::warn!("Option with invalid value encountered");
            continue;
        };
        if rv.insert(key, value).is_some() {
            macroquad::logging::warn!("Duplicate key encountered");
        }
    }
    rv
}

/// # Safety
/// - `in_ptr` must point to at least `in_len` bytes of initialized readable memory
/// - `out_ptr` must point to at least `out_len` bytes of writable memory
/// - This function initializes some `N` bytes of memory (0 <= N <= `out_len`) and returns N.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rustc_demangle_multiline(
    in_ptr: *const u8,
    in_len: usize,
    out_ptr: *mut MaybeUninit<u8>,
    out_len: usize,
) -> usize {
    // Safety: caller promises that (begin, length) forms a valid slice
    let bytes = unsafe { core::slice::from_raw_parts(in_ptr, in_len) };

    let s = String::from_utf8_lossy(bytes);
    let demangled = demangle_multiline_impl(&s);

    // Safety: caller promises that (out, max_out) forms a valid mut slice
    unsafe { write_bytes(demangled.as_bytes(), out_ptr, out_len) }
}

fn demangle_multiline_impl(s: &str) -> String {
    let mut rv = String::with_capacity(s.len());
    for line in s.lines() {
        // line looks something like: blahblah.wasm._ZN12penguin_game4game8run_game28_$u7b (http://...)
        // or: blahblah.wasm._ZN12penguin_game4game8run_game28_$u7b@http://...
        //
        // Stack traces are apparently not even standardized yet in JavaScript land.
        // [https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error/stack#description]
        //
        // This is quite a hack, but it's a best-effort thing anyway
        let Some((before_dotwasm, after_dotwasm)) = line.split_once(".wasm.") else {
            rv.push_str(line);
            rv.push('\n');
            continue;
        };
        let mangled_end = after_dotwasm.find(['(', '@']).unwrap_or(after_dotwasm.len());
        let mangled = after_dotwasm[..mangled_end].trim();
        let demangled = rustc_demangle::demangle(mangled).to_string();
        rv.push_str(before_dotwasm);
        rv.push_str(".wasm.");
        rv.push_str(&demangled);
        rv.push_str(&after_dotwasm[mangled_end..]);
        rv.push('\n');
    }
    rv
}

/// # Safety
/// We will write `0..=max_out` bytes to out. The number of bytes written will be returned.
unsafe fn write_bytes(input: &[u8], out: *mut MaybeUninit<u8>, max_out: usize) -> usize {
    let out_size = input.len().min(max_out);

    // SAFETY: this is always safe; MU<u8> has the same layout as u8
    let input: &[MaybeUninit<u8>] =
        unsafe { core::slice::from_raw_parts(input.as_ptr().cast(), out_size) };

    // SAFETY: caller promised that this is OK
    let out_slice = unsafe { core::slice::from_raw_parts_mut(out, out_size) };
    out_slice.copy_from_slice(input);
    out_size
}
