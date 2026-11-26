use sa_emit_js::unbound::{
    is_runtime_extern, trap_code, trap_message, unbound_stub_assignment, RUNTIME_EXTERNS,
};
use sa_emit_js::{emit_js, JsOptions};
use sa_ir::{Function, FunctionKind, Program, Signature, SourceLoc};

fn program_with_externs(names: &[&str]) -> Program {
    let mut out = Program::new();
    for name in names {
        let mut sig = Signature::new(*name);
        sig.kind = FunctionKind::External;
        out.externs.push(sig);
    }
    out
}

#[test]
fn j20_unbound_stub_traps_loudly() {
    let line = unbound_stub_assignment("hash_new");
    assert_eq!(
        line,
        "SA_EXTERNS[\"hash_new\"] = function () { sa_panic(2999, \"extern not linked: hash_new\"); };"
    );
    assert!(line.contains("sa_panic("));
    assert!(!line.contains("return 0"));
}

#[test]
fn j20_trap_message_is_named_and_escaped() {
    assert_eq!(trap_message("hash_new"), "\"extern not linked: hash_new\"");
    assert_eq!(trap_code(), 2999);
    assert_eq!(
        trap_message("call"),
        "\"extern not linked: sa_fn_call\"",
        "name is the emitted JS function name (js_func_name), not the raw ident"
    );
    assert_eq!(
        trap_message("a\"b\\c"),
        "\"extern not linked: a_b_c\"",
        "js_func_name sanitizes the name before js_string quotes the literal"
    );
}

#[test]
fn j20_runtime_externs_are_the_bound_four() {
    assert_eq!(
        RUNTIME_EXTERNS,
        [
            "sa_print_bytes",
            "sa_js_value_release",
            "sa_js_make_callback",
            "sa_js_callback_poll"
        ]
    );
    for name in RUNTIME_EXTERNS {
        assert!(is_runtime_extern(name));
    }
    for name in ["hash_new", "dlopen", "fd_open", "memcpy", ""] {
        assert!(!is_runtime_extern(name), "{name} must not be treated as bound");
    }
}

#[test]
fn j20_unbound_stubs_are_per_extern_and_deterministic() {
    let a = unbound_stub_assignment("dlopen");
    let b = unbound_stub_assignment("dlsym");
    assert_ne!(a, b);
    assert!(a.contains("SA_EXTERNS[\"dlopen\"]"));
    assert!(a.contains("extern not linked: dlopen"));
    assert!(b.contains("SA_EXTERNS[\"dlsym\"]"));
    assert!(b.contains("extern not linked: dlsym"));
    assert_eq!(a, unbound_stub_assignment("dlopen"));
}

#[test]
fn j20_emit_js_replaces_silent_stub_with_trap() {
    let program = program_with_externs(&["hash_new"]);
    let js = emit_js(&program, &JsOptions::default()).expect("emit must succeed");
    assert!(js.contains(
        "SA_EXTERNS[\"hash_new\"] = function () { sa_panic(2999, \"extern not linked: hash_new\"); };"
    ));
    assert!(!js.contains("return 0; };"), "silent stub must be gone");
    assert!(js.contains("call(\"hash_new\", [])"), "entry still dispatches by name");
}

#[test]
fn j20_bound_extern_surface_stays_unchanged() {
    let mut program = program_with_externs(&[
        "sa_print_bytes",
        "sa_js_value_release",
        "sa_js_make_callback",
        "sa_js_callback_poll",
        "fd_open",
        "hash_new",
        "my_bound_fn",
    ]);
    program
        .functions
        .push(Function::new(Signature::new("my_bound_fn"), SourceLoc::unknown()));
    let js = emit_js(&program, &JsOptions::default()).expect("emit must succeed");

    assert_eq!(js.matches("sa_panic(2999, \"extern not linked:").count(), 1);
    assert!(js.contains("SA_EXTERNS[\"hash_new\"] = function () { sa_panic(2999"));
    assert!(js.contains("function fd_open(path)"), "builtin shim still emitted");
    assert!(js.contains("SA_EXTERNS[\"fd_open\"] = fd_open;"));
    assert!(!js.contains("extern not linked: fd_open"));
    assert!(!js.contains("extern not linked: sa_print_bytes"));
    assert!(!js.contains("extern not linked: sa_js_value_release"));
    assert!(!js.contains("extern not linked: sa_js_make_callback"));
    assert!(!js.contains("extern not linked: sa_js_callback_poll"));
    assert!(!js.contains("extern not linked: my_bound_fn"), "class ① user fn stays bound");
}
