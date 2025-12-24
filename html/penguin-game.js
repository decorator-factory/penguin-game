import * as mq from "./miniquad.js";

const options = {
  url_fragment: document.location.hash,
  asset_root: document.baseURI,
};

const encodedOptions = new TextEncoder().encode(
  Object.entries(options).map(([k, v]) => {
    // The only way to put a `\x00` or `\x01` in a UTF8 string is using
    // the corresponding character.
    if (k.includes("\x00") || k.includes("\x01")) throw new Error("k contains bad byte")
    if (v.includes("\x00") || v.includes("\x01")) throw new Error("v contains bad byte")
    return k + "\x01" + v
  }).join("\x00")
);

export async function init({
  errorElement,
  backtraceElement,
  canvasElement,
}) {
  mq.set_canvas(canvasElement);

  mq.define_ffi_function("penguin_read_options", ffiReadOptions);
  mq.define_ffi_function("penguin_monotonic_millis", function () {
    return performance.now()
  })

  mq.set_demangler(demangleMultilineString);
  mq.add_panic_handler((message, backtrace) => {
    backtraceElement.innerText = message + "\n--------\n" + backtrace;
    errorElement.classList.add("show");
  });

  await mq.load("penguin-game.wasm");
  console.log("Loaded penguin-game.wasm successfully")
}

function ffiReadOptions(out_ptr, max_length) {
  const len = encodedOptions.byteLength;
  if (max_length < len) {
    throw new Error("Options are too long!");
  }
  new Uint8Array(mq.wasm_memory.buffer, out_ptr, len).set(encodedOptions);
  return len;
}

function demangleMultilineString(string) {
  // NOTE: this function will intentionally leak some memory
  const utf8 = new TextEncoder().encode(string);

  const in_size = utf8.byteLength;
  const in_ptr = mq.wasm_exports.allocate_vec_u8(in_size);

  const out_size = utf8.byteLength * 4
  const out_ptr = mq.wasm_exports.allocate_vec_u8(out_size);

  new Uint8Array(mq.wasm_memory.buffer, in_ptr, utf8.byteLength).set(utf8);
  const bytes_written = mq.wasm_exports.rustc_demangle_multiline(
    in_ptr, in_size,
    out_ptr, out_size,
  );

  const out_array = new Uint8Array(mq.wasm_memory.buffer, out_ptr, bytes_written);
  return new TextDecoder().decode(out_array)
}
