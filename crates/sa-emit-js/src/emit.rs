use crate::buf::Buf;
use crate::ident::{const_name, js_float, js_func_name, js_int, js_string};
use crate::lower::{build_ctx, emit_function, ByteConsts};
use crate::runtime;
use crate::JsOptions;
use sa_ir::{ConstValue, Diagnostic, PrimType, Program, Trap};
use std::collections::HashMap;

struct Export {
    js: String,
    original: String,
}

pub fn emit(program: &Program, opts: &JsOptions) -> Result<String, Vec<Diagnostic>> {
    let mut function_names: HashMap<String, String> = HashMap::new();
    function_names.extend(program.functions.iter().map(|f| (f.sig.name.clone(), js_func_name(&f.sig.name))));

    let entry = match &opts.entry {
        Some(name) => {
            if !function_names.contains_key(name) && !program.has_extern(name) {
                return Err(vec![Diagnostic::new(
                    Trap::RuntimeError,
                    format!("entry function not found: {name}"),
                    sa_ir::SourceLoc::unknown(),
                )]);
            }
            Some(name.clone())
        }
        None => program
            .entry_function()
            .map(|f| f.sig.name.clone())
            .or_else(|| program.externs.first().map(|e| e.name.clone())),
    };

    let mut bytes = ByteConsts::from_program(program);
    let mut buf = Buf::new();
    buf.raw(&runtime::prelude(opts.memory_pages));

    emit_consts(program, &bytes, &mut buf);

    for f in &program.functions {
        let mut ctx = build_ctx(program, f, &mut bytes, opts.debug);
        emit_function(f, &mut buf, &mut ctx)?;
    }

    emit_footer(program, opts, &function_names, entry.as_deref(), &mut buf);
    Ok(buf.s)
}

fn emit_consts(program: &Program, bytes: &ByteConsts, buf: &mut Buf) {
    for c in &program.consts {
        let name = const_name(&c.name);
        match &c.value {
            ConstValue::Int(v) => {
                let float_ty = matches!(c.ty, Some(PrimType::F32) | Some(PrimType::F64));
                let lit = if float_ty {
                    js_float(*v as f64)
                } else {
                    js_int(*v)
                };
                buf.push_indent(0, &format!("const {name} = {lit};"));
            }
            ConstValue::Float(f) => buf.push_indent(0, &format!("const {name} = {};", js_float(*f))),
            ConstValue::Bool(b) => buf.push_indent(0, &format!("const {name} = {};", *b as u8)),
            ConstValue::Null | ConstValue::Undef => buf.push_indent(0, &format!("const {name} = 0;")),
            other => {
                if let Some(data) = const_bytes(other) {
                    emit_bytes_const(&name, &data, buf);
                } else {
                    buf.push_indent(0, &format!("const {name} = 0;"));
                }
            }
        }
    }
    for (name, data) in bytes.items() {
        emit_bytes_const(name, data, buf);
    }
}

fn emit_bytes_const(name: &str, data: &[u8], buf: &mut Buf) {
    let ptr = format!("{name}_ptr");
    buf.push_indent(0, &format!("const {ptr} = sa_alloc({});", data.len()));
    buf.push_indent(0, &format!("const {name} = {ptr};"));
    for (i, b) in data.iter().enumerate() {
        buf.push_indent(0, &format!("U8[{ptr} + {i}] = {b};"));
    }
}

fn const_bytes(value: &ConstValue) -> Option<Vec<u8>> {
    match value {
        ConstValue::Bytes(b) => Some(b.clone()),
        ConstValue::Utf8(s) => Some(s.as_bytes().to_vec()),
        ConstValue::Aggregate(v) | ConstValue::VTable(v) => {
            let mut out = Vec::new();
            for item in v {
                out.extend(const_bytes(item)?);
            }
            Some(out)
        }
        ConstValue::Bool(b) => Some(vec![*b as u8]),
        ConstValue::Int(v) => Some((*v as u64).to_le_bytes().to_vec()),
        ConstValue::Float(f) => Some(f.to_le_bytes().to_vec()),
        ConstValue::Null | ConstValue::Undef => Some(vec![0]),
    }
}

fn emit_footer(
    program: &Program,
    opts: &JsOptions,
    function_names: &HashMap<String, String>,
    entry: Option<&str>,
    buf: &mut Buf,
) {
    let exports: Vec<Export> = program
        .functions
        .iter()
        .map(|f| Export {
            js: js_func_name(&f.sig.name),
            original: f.sig.name.clone(),
        })
        .collect();

    let funcs: Vec<&str> = exports.iter().map(|e| e.js.as_str()).collect();
    buf.push_indent(0, &format!("SA_FUNCS = [{}];", funcs.join(", ")));
    for e in &exports {
        buf.push_indent(
            0,
            &format!("SA_FUNCS_BY_NAME[{}] = {};", js_string(&e.original), e.js),
        );
    }

    buf.push_indent(0, "SA_EXTERNS[\"sa_print_bytes\"] = sa_print_bytes;");
    for sig in &program.externs {
        if function_names.contains_key(&sig.name) || sig.name == "sa_print_bytes" {
            continue;
        }
        buf.push_indent(
            0,
            &format!(
                "SA_EXTERNS[{}] = function () {{ return 0; }};",
                js_string(&sig.name)
            ),
        );
    }

    match opts.format {
        crate::JsFormat::Esm => {
            if exports.is_empty() {
                buf.push_indent(0, "export {};");
            } else {
                let names: Vec<&str> = exports.iter().map(|e| e.js.as_str()).collect();
                buf.push_indent(0, &format!("export {{ {} }};", names.join(", ")));
            }
        }
        crate::JsFormat::Cjs => {
            let pairs: Vec<String> = exports
                .iter()
                .map(|e| format!("{}: {}", js_string(&e.original), e.js))
                .collect();
            buf.push_indent(0, &format!("module.exports = {{ {} }};", pairs.join(", ")));
        }
    }

    if let Some(name) = entry {
        let call = match function_names.get(name) {
            Some(js) => format!("{js}()"),
            None => format!("call({}, [])", js_string(name)),
        };
        let guard = match opts.format {
            crate::JsFormat::Esm => "typeof process !== 'undefined' && process.argv && process.argv[1] && process.argv[1].endsWith('.mjs')",
            crate::JsFormat::Cjs => "typeof require !== 'undefined' && typeof module !== 'undefined' && require.main === module",
        };
        buf.push_indent(0, &format!("if ({guard}) {{"));
        buf.push_indent(1, &format!("const code = {call};"));
        buf.push_indent(1, "if (typeof process !== 'undefined') process.exit(Number(code) | 0);");
        buf.push_indent(0, "}");
    }
}
