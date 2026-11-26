//! J20 — unbound extern stub generation.
//!
//! sci oracle `emit_js/funcs.zig:150-183`: a known shim gets a real binding,
//! every other extern gets `function () { __sa_trap("extern not linked: <js>"); }`.
//! sars mirror: `sa_panic(2999, "extern not linked: <js>")` (see `runtime.rs`).
//! This module encodes only the distinction that J20 changes: class ② (four
//! prelude externs) and class ⑤ (everything else). Classes ①③④ stay in
//! `emit.rs` and are left byte-identical.

use crate::ident::{js_func_name, js_string};

/// Externs bound by the runtime prelude (`emit.rs` footer, four assignments).
pub const RUNTIME_EXTERNS: [&str; 4] = [
    "sa_print_bytes",
    "sa_js_value_release",
    "sa_js_make_callback",
    "sa_js_callback_poll",
];

/// `Trap::RuntimeError` code — reused by `sa_panic`, no new trap variant.
pub const TRAP_CODE: u32 = 2999;

/// True for class ②: the extern already has a prelude binding.
pub fn is_runtime_extern(name: &str) -> bool {
    RUNTIME_EXTERNS.contains(&name)
}

/// Runtime trap code used by the unbound-extern stub.
pub fn trap_code() -> u32 {
    TRAP_CODE
}

/// Escaped JS string literal: `"extern not linked: <js_func_name>"`, matching
/// sci `funcs.zig:180-182` verbatim in the message body.
pub fn trap_message(name: &str) -> String {
    js_string(&format!("extern not linked: {}", js_func_name(name)))
}

/// Full `SA_EXTERNS` binding line for an unbound extern (class ⑤).
pub fn unbound_stub_assignment(name: &str) -> String {
    format!(
        "SA_EXTERNS[{}] = function () {{ sa_panic({}, {}); }};",
        js_string(name),
        TRAP_CODE,
        trap_message(name)
    )
}
