//! WebAssembly shell. Stub; M5 wires up canvas + input + render loop via wasm-bindgen.

#[cfg(target_arch = "wasm32")]
mod platform;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"antidote-wasm: M1 stub — render loop arrives in M5.".into());
}
