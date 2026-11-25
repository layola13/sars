use super::*;
use sa_ir::{
    Address, Block, ConstDecl, ConstValue, Function, ImportDecl, Instruction, InstructionKind,
    OpKind, Operand, PrimType, Program, Signature, SourceLoc, Trap,
};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn ins(kind: InstructionKind) -> Instruction {
    Instruction::new(kind, SourceLoc::unknown(), "")
}

fn dest(kind: InstructionKind, name: &str) -> Instruction {
    ins(kind).with_dest(name)
}

fn typed(kind: InstructionKind, name: &str, ty: sa_ir::PrimType) -> Instruction {
    dest(kind, name).with_ty(ty)
}

fn block(label: &str, instrs: Vec<Instruction>) -> Block {
    Block {
        label: label.to_string(),
        instrs,
        loc: SourceLoc::unknown(),
    }
}

fn function(name: &str, blocks: Vec<Block>) -> Function {
    let mut sig = Signature::new(name);
    sig.return_ty = sa_ir::PrimType::I32;
    let mut f = Function::new(sig, SourceLoc::unknown());
    f.blocks = blocks;
    f
}

fn program(functions: Vec<Function>) -> Program {
    let mut p = Program::new();
    p.functions = functions;
    p
}

fn balanced(s: &str) -> bool {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in s.chars() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => quote = Some(c),
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0 && quote.is_none()
}

fn no_comments(s: &str) -> bool {
    !s.contains("//") && !s.contains("/*")
}

fn node_check(js: &str, esm: bool) {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let ext = if esm { "mjs" } else { "cjs" };
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("sa_emit_js_{}_{id}.{ext}", std::process::id()));
    std::fs::write(&path, js).unwrap();
    let out = Command::new("node").arg("--check").arg(&path).output().unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "node --check failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn default_options_match_spec() {
    let o = JsOptions::default();
    assert_eq!(o.format, JsFormat::Esm);
    assert!(!o.debug);
    assert!(o.entry.is_none());
    assert_eq!(o.memory_pages, 2);
}

#[test]
fn empty_program_emits_valid_esm() {
    let js = emit_js(&Program::new(), &JsOptions::default()).unwrap();
    assert!(js.contains("const SA_PAGES = 2;"));
    assert!(js.contains("const memory = new ArrayBuffer(SA_SIZE);"));
    assert!(js.contains("function sa_alloc("));
    assert!(js.contains("function sa_load("));
    assert!(js.contains("function sa_store("));
    assert!(js.contains("export {};"));
    assert!(balanced(&js));
    assert!(no_comments(&js));
    node_check(&js, true);
}

#[test]
fn alloc_store_load_add_return() {
    let body = vec![
        dest(InstructionKind::Alloc { size: Operand::Int(4) }, "p"),
        ins(InstructionKind::Store {
            addr: Address::new(Operand::reg("p"), 0),
            value: Operand::Int(42),
        }),
        typed(InstructionKind::Load { addr: Address::new(Operand::reg("p"), 0) }, "v", sa_ir::PrimType::I32),
        dest(InstructionKind::Op { op: OpKind::Add, args: vec![Operand::reg("v"), Operand::Int(1)] }, "r"),
        ins(InstructionKind::Return { value: Some(Operand::reg("r")) }),
    ];
    let p = program(vec![function("main", vec![block("entry", body)])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("function main("));
    assert!(js.contains("case 0: {"));
    assert!(js.contains("sa_alloc(4)"));
    assert!(js.contains("sa_store("));
    assert!(js.contains("sa_load("));
    assert!(js.contains("sa_add("));
    assert!(js.contains("return r_r;"));
    assert!(js.contains("export { main };"));
    assert!(balanced(&js));
    assert!(no_comments(&js));
    node_check(&js, true);
}

#[test]
fn branches_emit_phi_style_bb_dispatch() {
    let body = vec![ins(InstructionKind::Br {
        cond: Operand::Int(1),
        then_label: "L1".to_string(),
        else_label: "L2".to_string(),
    })];
    let p = program(vec![function("main", vec![
        block("entry", body),
        block("L1", vec![ins(InstructionKind::Return { value: Some(Operand::Int(1)) })]),
        block("L2", vec![ins(InstructionKind::Return { value: Some(Operand::Int(2)) })]),
    ])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("bb = (1) ? 1 : 2; break;"));
    assert!(js.contains("case 1: {"));
    assert!(js.contains("case 2: {"));
    assert!(balanced(&js));
    node_check(&js, true);
}

#[test]
fn jump_and_br_null() {
    let p = program(vec![function("main", vec![
        block("entry", vec![ins(InstructionKind::BrNull {
            cond: Operand::reg("x"),
            target: "L1".to_string(),
            else_target: None,
        })]),
        block("L1", vec![ins(InstructionKind::Jmp { target: "L2".to_string() })]),
        block("L2", vec![ins(InstructionKind::Return { value: None })]),
    ])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("if ((r_x) == 0) { bb = 1; break; }"));
    assert!(js.contains("bb = 2; break;"));
    assert!(balanced(&js));
    node_check(&js, true);
}

#[test]
fn fallible_return_and_try() {
    let mut f = function("main", vec![block("entry", vec![
        dest(InstructionKind::Assign { src: Operand::Int(5) }, "v"),
        dest(InstructionKind::Try { src: Operand::reg("v") }, "u"),
        ins(InstructionKind::Return { value: Some(Operand::reg("u")) }),
    ])]);
    f.sig.return_fallible = true;
    let js = emit_js(&program(vec![f]), &JsOptions::default()).unwrap();
    assert!(js.contains("sa_unwrap("));
    assert!(js.contains("return {v: r_u, ok: true};"));
    assert!(balanced(&js));
    node_check(&js, true);
}

#[test]
fn native_instruction_is_rejected() {
    let p = program(vec![function("main", vec![block("entry", vec![
        ins(InstructionKind::Native { text: "evil()".to_string() }),
    ])])]);
    let err = emit_js(&p, &JsOptions::default()).unwrap_err();
    assert_eq!(err.len(), 1);
    assert_eq!(err[0].trap, Trap::UnsupportedInstruction);
}

#[test]
fn empty_inline_asm_is_a_no_op() {
    let p = program(vec![function("main", vec![block("entry", vec![
        ins(InstructionKind::Native {
            text: "call void asm sideeffect \"\", \"\" ()".to_string(),
        }),
        ins(InstructionKind::Return { value: Some(Operand::Int(0)) }),
    ])])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(!js.contains("unsupported native"), "{}", &js);
    node_check(&js, true);
}

#[test]
fn utf8_const_allocates_memory() {
    let mut p = program(vec![function("main", vec![block("entry", vec![
        ins(InstructionKind::Return { value: Some(Operand::Int(0)) }),
    ])])]);
    p.consts.push(ConstDecl::new("s", ConstValue::Utf8("hi".to_string())));
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("const C_s_ptr = sa_alloc(2);"));
    assert!(js.contains("U8[C_s_ptr + 0] = 104;"));
    assert!(js.contains("U8[C_s_ptr + 1] = 105;"));
    assert!(balanced(&js));
    node_check(&js, true);
}

#[test]
fn inline_bytes_operand_becomes_const() {
    let p = program(vec![function("main", vec![block("entry", vec![
        dest(InstructionKind::Assign { src: Operand::Bytes(vec![1, 2, 3]) }, "b"),
        ins(InstructionKind::Return { value: Some(Operand::reg("b")) }),
    ])])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("const C_B0_ptr = sa_alloc(3);"));
    assert!(js.contains("r_b = C_B0;"));
    node_check(&js, true);
}

#[test]
fn cjs_footer_and_entry_override() {
    let p = program(vec![function("compute", vec![block("entry", vec![
        ins(InstructionKind::Return { value: Some(Operand::Int(7)) }),
    ])])]);
    let opts = JsOptions {
        format: JsFormat::Cjs,
        entry: Some("compute".to_string()),
        ..JsOptions::default()
    };
    let js = emit_js(&p, &opts).unwrap();
    assert!(js.contains("module.exports = { \"compute\": compute };"));
    assert!(js.contains("require.main === module"));
    assert!(js.contains("const code = compute();"));
    assert!(js.contains("process.exit(Number(code) | 0);"));
    assert!(balanced(&js));
    node_check(&js, false);
}

#[test]
fn bad_entry_is_diagnostic() {
    let opts = JsOptions {
        entry: Some("missing".to_string()),
        ..JsOptions::default()
    };
    let err = emit_js(&Program::new(), &opts).unwrap_err();
    assert_eq!(err[0].trap, Trap::RuntimeError);
}

#[test]
fn all_ops_lower_to_helpers() {
    let ops = [
        OpKind::Add, OpKind::Sub, OpKind::Mul, OpKind::SDiv, OpKind::UDiv,
        OpKind::SRem, OpKind::URem, OpKind::Neg, OpKind::And, OpKind::Or,
        OpKind::Xor, OpKind::Shl, OpKind::LShr, OpKind::AShr, OpKind::Not,
        OpKind::Eq, OpKind::Ne, OpKind::Slt, OpKind::Sle, OpKind::Sgt,
        OpKind::Sge, OpKind::Ult, OpKind::Ule, OpKind::Ugt, OpKind::Uge,
        OpKind::Div, OpKind::Rem, OpKind::Gt, OpKind::Lt, OpKind::Shr,
        OpKind::FAdd, OpKind::FSub, OpKind::FMul, OpKind::FDiv, OpKind::FNeg,
        OpKind::FcmpEq, OpKind::FcmpNe, OpKind::FcmpLt, OpKind::FcmpLe,
        OpKind::FcmpGt, OpKind::FcmpGe, OpKind::Trunc, OpKind::Zext,
        OpKind::Sext, OpKind::Fptosi, OpKind::Sitofp, OpKind::Uitofp,
        OpKind::Fptrunc, OpKind::Fpext, OpKind::Bitcast, OpKind::AddV128,
        OpKind::SubV128, OpKind::MulV128, OpKind::ShuffleV128,
        OpKind::ExtractLane, OpKind::InsertLane,
    ];
    let mut instrs = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        instrs.push(typed(
            InstructionKind::Op {
                op: *op,
                args: vec![Operand::Int(1), Operand::Int(2), Operand::Int(3)],
            },
            &format!("r{i}"),
            sa_ir::PrimType::I64,
        ));
    }
    instrs.push(ins(InstructionKind::Return { value: Some(Operand::Int(0)) }));
    let p = program(vec![function("main", vec![block("entry", instrs)])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    for expected in [
        "sa_add(", "sa_sub(", "sa_mul(", "sa_sdiv(", "sa_udiv(", "sa_srem(", "sa_urem(",
        "sa_neg(", "sa_band(", "sa_bor(", "sa_bxor(", "sa_shl(", "sa_lshr(", "sa_ashr(",
        "sa_bnot(", "sa_eq(", "sa_ne(", "sa_slt(", "sa_sle(", "sa_sgt(", "sa_sge(",
        "sa_ult(", "sa_ule(", "sa_ugt(", "sa_uge(", "sa_div(", "sa_rem(", "sa_gt(",
        "sa_lt(", "sa_shr(", "sa_fadd(", "sa_fsub(", "sa_fmul(", "sa_fdiv(", "sa_fneg(",
        "sa_fcmp_eq(", "sa_fcmp_ne(", "sa_fcmp_lt(", "sa_fcmp_le(", "sa_fcmp_gt(",
        "sa_fcmp_ge(", "sa_cvt(", "sa_v128(", "sa_shuffle_v128(", "sa_extract_lane(",
        "sa_insert_lane(",
    ] {
        assert!(js.contains(expected), "missing {expected}");
    }
    assert!(balanced(&js));
    assert!(no_comments(&js));
    node_check(&js, true);
}

#[test]
fn externs_get_shims_and_calls_dispatch() {
    let mut p = program(vec![function("main", vec![block("entry", vec![
        dest(InstructionKind::Call {
            target: "foreign".to_string(),
            args: vec![],
        }, "x"),
        ins(InstructionKind::Return { value: Some(Operand::reg("x")) }),
    ])])]);
    let mut sig = Signature::new("foreign");
    sig.kind = sa_ir::FunctionKind::External;
    p.externs.push(sig);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("call(\"foreign\", [])"));
    assert!(js.contains("SA_EXTERNS[\"foreign\"] = function () { return 0; };"));
    assert!(js.contains("SA_EXTERNS[\"sa_print_bytes\"] = sa_print_bytes;"));
    node_check(&js, true);
}

#[test]
fn c_group_externs_get_real_contract_shims() {
    let names = [
        "fd_open", "fd_read", "fd_close", "pthread_spawn", "pthread_drop",
        "pthread_join", "sa_pthread_join", "sqlite3_prepare", "sqlite3_step",
        "sqlite3_finalize", "sa_std_compress_supported", "sa_std_compress_encode",
        "sa_std_compress_decode", "sa_std_compress_buffer_data",
        "sa_std_compress_buffer_len", "sa_std_compress_buffer_free",
        "sa_std_multipart_parse", "sa_std_multipart_count",
        "sa_std_multipart_part_body", "sa_std_multipart_part_name",
        "sa_std_multipart_part_filename", "sa_std_multipart_part_content_type",
        "sa_std_multipart_part_header", "sa_std_multipart_builder_new",
        "sa_std_multipart_builder_free", "sa_std_multipart_builder_add_field",
        "sa_std_multipart_builder_add_file", "sa_std_multipart_builder_boundary",
        "sa_std_multipart_builder_content_type", "sa_std_multipart_builder_finish",
        "sa_std_multipart_buffer_data", "sa_std_multipart_buffer_len",
        "sa_std_multipart_buffer_free",
    ];
    let mut p = program(vec![function("main", vec![block("entry", vec![
        ins(InstructionKind::Return { value: Some(Operand::Int(0)) }),
    ])])]);
    for name in names {
        let mut sig = Signature::new(name);
        sig.kind = sa_ir::FunctionKind::External;
        p.externs.push(sig);
    }
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    for name in names {
        assert!(js.contains(&format!("function {name}(")), "missing shim {name}");
        assert!(
            js.contains(&format!("SA_EXTERNS[\"{name}\"] = {name};")),
            "missing dispatch {name}"
        );
        assert!(
            !js.contains(&format!("SA_EXTERNS[\"{name}\"] = function () {{ return 0; }};")),
            "stub not replaced {name}"
        );
    }
    assert!(js.contains("globalThis.__sa_next_fd = (globalThis.__sa_next_fd || 3);"));
    assert!(js.contains("if (i >= 0) SA_FUNCS[i](arg);"));
    assert!(js.contains("function sqlite3_step(stmt) { return 100; }"));
    assert!(js.contains("p.getBuiltinModule(\"node:zlib\")"));
    assert!(js.contains("text.split(\"--\" + boundary)"));
    assert!(balanced(&js));
    assert!(no_comments(&js));
    node_check(&js, true);
}

#[test]
fn extra_destinations_are_declared_and_bound() {
    let mut exchange = typed(
        InstructionKind::Cmpxchg {
            addr: Address::new(Operand::reg("p"), 0),
            expected: Operand::Int(0),
            replacement: Operand::Int(1),
            order: sa_ir::AtomicOrdering::SeqCst,
        },
        "old",
        sa_ir::PrimType::I32,
    );
    exchange.extra_dests = vec!["ok".into()];
    let p = program(vec![function("main", vec![block("entry", vec![
        dest(InstructionKind::Alloc { size: Operand::Int(8) }, "p"),
        exchange,
        ins(InstructionKind::Return { value: Some(Operand::reg("ok")) }),
    ])])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("let r_ok = 0;"), "{js}");
    assert!(js.contains("r_ok = (r_old === sa_num(0) ? 1 : 0);"), "{js}");
    node_check(&js, true);
}

#[test]
fn atomics_and_fences_are_valid() {
    let p = program(vec![function("main", vec![block("entry", vec![
        dest(InstructionKind::Alloc { size: Operand::Int(8) }, "p"),
        typed(InstructionKind::AtomicLoad {
            addr: Address::new(Operand::reg("p"), 0),
            order: sa_ir::AtomicOrdering::SeqCst,
        }, "a", sa_ir::PrimType::I32),
        ins(InstructionKind::AtomicStore {
            addr: Address::new(Operand::reg("p"), 0),
            value: Operand::Int(3),
            order: sa_ir::AtomicOrdering::Relaxed,
        }),
        typed(InstructionKind::Cmpxchg {
            addr: Address::new(Operand::reg("p"), 0),
            expected: Operand::Int(3),
            replacement: Operand::Int(4),
            order: sa_ir::AtomicOrdering::AcqRel,
        }, "c", sa_ir::PrimType::I32),
        typed(InstructionKind::AtomicRmw {
            op: sa_ir::AtomicRmwOp::Umin,
            addr: Address::new(Operand::reg("p"), 0),
            value: Operand::Int(2),
            order: sa_ir::AtomicOrdering::SeqCst,
        }, "m", sa_ir::PrimType::I32),
        ins(InstructionKind::Fence { order: sa_ir::AtomicOrdering::SeqCst }),
        ins(InstructionKind::Return { value: Some(Operand::reg("a")) }),
    ])])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("Atomics.load("));
    assert!(js.contains("Atomics.store("));
    assert!(js.contains("Atomics.compareExchange("));
    assert!(js.contains("sa_atomic_rmw(\"umin\""));
    assert!(balanced(&js));
    node_check(&js, true);
}

#[test]
fn varargs_and_pointers_lower() {
    let p = program(vec![function("main", vec![block("entry", vec![
        ins(InstructionKind::VaStart),
        dest(InstructionKind::VaArg { src: Operand::Int(0) }, "a"),
        ins(InstructionKind::VaEnd),
        dest(InstructionKind::PtrAdd { base: Operand::reg("a"), offset: Operand::Int(8) }, "p"),
        dest(InstructionKind::Borrow { src: Operand::reg("p"), mutable: false }, "q"),
        dest(InstructionKind::Move { src: Operand::reg("q") }, "z"),
        ins(InstructionKind::Return { value: Some(Operand::reg("z")) }),
    ])])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("SA_VA = SA_VARARGS.slice();"));
    assert!(js.contains("sa_ptr_add("));
    assert!(js.contains("SA_VA = [];"));
    assert!(balanced(&js));
    node_check(&js, true);
}

#[test]
fn runs_emitted_js_under_node() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let p = program(vec![function("main", vec![block("entry", vec![
        dest(InstructionKind::Alloc { size: Operand::Int(3) }, "p"),
        ins(InstructionKind::Store {
            addr: Address::new(Operand::reg("p"), 0),
            value: Operand::Int(72),
        }),
        ins(InstructionKind::Store {
            addr: Address::new(Operand::reg("p"), 1),
            value: Operand::Int(105),
        }),
        dest(InstructionKind::Call {
            target: "sa_print_bytes".to_string(),
            args: vec![
                sa_ir::CallArg::new(sa_ir::CapPrefix::ByValue, Operand::reg("p")),
                sa_ir::CallArg::new(sa_ir::CapPrefix::ByValue, Operand::Int(2)),
            ],
        }, "n"),
        ins(InstructionKind::Return { value: Some(Operand::Int(0)) }),
    ])])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("sa_emit_js_run_{}_{id}.mjs", std::process::id()));
    std::fs::write(&path, &js).unwrap();
    let out = Command::new("node").arg(&path).output().unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "Hi");
}

#[test]
fn source_files_within_line_limit() {
    fn lines(s: &str) -> usize {
        s.lines().count()
    }
    assert!(lines(include_str!("lib.rs")) <= 1000);
    assert!(lines(include_str!("buf.rs")) <= 1000);
    assert!(lines(include_str!("ident.rs")) <= 1000);
    assert!(lines(include_str!("runtime.rs")) <= 1000);
    assert!(lines(include_str!("lower.rs")) <= 1000);
    assert!(lines(include_str!("emit.rs")) <= 1000);
}

#[test]
fn npm_bind_emits_import_and_wrapper() {
    let mut p = program(vec![function("main", vec![block(
        "entry",
        vec![ins(InstructionKind::Return { value: None })],
    )])]);
    let mut sig = sa_ir::Signature::new("npm_totalmem");
    sig.kind = sa_ir::FunctionKind::External;
    sig.return_ty = PrimType::U64;
    p.externs.push(sig);
    p.npm_binds.push(sa_ir::NpmBindDecl {
        extern_name: "npm_totalmem".into(),
        spec: "node:os".into(),
        target: "totalmem".into(),
        shape: sa_ir::NpmBindShape::Call,
        loc: SourceLoc::unknown(),
    });
    let js = super::emit_js(&p, &super::JsOptions::default()).unwrap();
    assert!(js.contains("import { totalmem as __sa_npm_0 } from \"node:os\";"));
    assert!(js.contains("SA_EXTERNS[\"npm_totalmem\"] = function () {"));
    assert!(js.contains("return __sa_npm_0();"));
}

#[test]
fn no_dest_writes_use_normalized_sa_tmp() {
    let p = program(vec![function("main", vec![block("entry", vec![
        ins(InstructionKind::Alloc { size: Operand::Int(3) }),
        ins(InstructionKind::Return { value: Some(Operand::Int(0)) }),
    ])])]);
    let js = emit_js(&p, &JsOptions::default()).unwrap();
    assert!(js.contains("let r_sa_tmp = 0;"), "{}", &js);
    assert!(js.contains("r_sa_tmp = sa_alloc(3)"), "{}", &js);
    let bytes = js.as_bytes();
    let needle = b"sa_tmp";
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let prev = if i == 0 { b' ' } else { bytes[i - 1] };
            assert_eq!(prev, b'_', "bare sa_tmp at byte {i}: {}", &js);
            i += needle.len();
        } else {
            i += 1;
        }
    }
}

#[test]
fn unused_imports_placeholder() {
    let _ = ImportDecl::new("x", SourceLoc::unknown());
}
