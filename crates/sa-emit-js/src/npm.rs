use crate::buf::Buf;
use crate::ident::js_string;
use crate::JsOptions;
use sa_ir::{NpmBindShape, PrimType, Program, Signature};
use std::collections::HashMap;

pub struct NpmInfo {
    pub alias: String,
    pub member: Option<String>,
    pub shape: NpmBindShape,
    pub bytes_view: bool,
}

pub fn collect(program: &Program) -> HashMap<String, NpmInfo> {
    let mut out = HashMap::new();
    for (index, bind) in program.npm_binds.iter().enumerate() {
        let (target, keywords) = match bind.target.split_once(':') {
            Some((target, keywords)) => (target, keywords),
            None => (bind.target.as_str(), ""),
        };
        let bytes_view = keywords.split(':').any(|kw| kw == "bytes" || kw == "f32view");
        let member = if matches!(bind.shape, NpmBindShape::Method | NpmBindShape::Get) {
            Some(target.to_string())
        } else {
            target.split_once('.').map(|(_, member)| member.to_string())
        };
        out.insert(
            bind.extern_name.clone(),
            NpmInfo {
                alias: format!("__sa_npm_{index}"),
                member,
                shape: bind.shape,
                bytes_view,
            },
        );
    }
    out
}

pub fn emit_imports(
    program: &Program,
    opts: &JsOptions,
    infos: &HashMap<String, NpmInfo>,
    buf: &mut Buf,
) {
    for bind in &program.npm_binds {
        let info = match infos.get(&bind.extern_name) {
            Some(info) => info,
            None => continue,
        };
        if matches!(info.shape, NpmBindShape::Method | NpmBindShape::Get) {
            continue;
        }
        let spec = js_string(&bind.spec);
        let alias = &info.alias;
        let target = bind.target.split(':').next().unwrap_or(&bind.target);
        match opts.format {
            crate::JsFormat::Esm => match &info.member {
                Some(_) => buf.push_indent(0, &format!("import * as {alias} from {spec};")),
                None => buf.push_indent(
                    0,
                    &format!("import {{ {target} as {alias} }} from {spec};"),
                ),
            },
            crate::JsFormat::Cjs => match &info.member {
                Some(member) => {
                    buf.push_indent(0, &format!("const {alias} = require({spec}).{member};"));
                }
                None => buf.push_indent(
                    0,
                    &format!("const {alias} = require({spec}).{target};"),
                ),
            },
        }
    }
}

fn is_integer(ty: PrimType) -> bool {
    matches!(
        ty,
        PrimType::I8
            | PrimType::I16
            | PrimType::I32
            | PrimType::I64
            | PrimType::U8
            | PrimType::U16
            | PrimType::U32
            | PrimType::U64
    )
}

fn string_out(sig: &Signature, info: &NpmInfo) -> Option<(usize, usize, usize)> {
    if matches!(info.shape, NpmBindShape::Ctor) || !is_integer(sig.return_ty) {
        return None;
    }
    let params = &sig.params;
    let n = params.len();
    if n < 3 {
        return None;
    }
    if params[n - 3].ty == PrimType::Ptr
        && is_integer(params[n - 2].ty)
        && params[n - 1].ty == PrimType::Ptr
    {
        Some((n - 3, n - 2, n - 1))
    } else {
        None
    }
}

pub fn emit_wrapper(sig: &Signature, info: &NpmInfo, buf: &mut Buf) {
    let mut params: Vec<String> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    let out_slots = string_out(sig, info);
    let mut i = 0usize;
    if matches!(info.shape, NpmBindShape::Method | NpmBindShape::Get) {
        params.push("a0".to_string());
        args.push("sa_handle_get(a0)".to_string());
        i = 1;
    }
    while i < sig.params.len() {
        let pname = format!("a{i}");
        params.push(pname.clone());
        let next_is_len = i + 1 < sig.params.len()
            && is_integer(sig.params[i + 1].ty)
            && !out_slots.map(|(out, _, _)| out == i).unwrap_or(false);
        if sig.params[i].ty == PrimType::Ptr && next_is_len {
            let len_name = format!("a{}", i + 1);
            params.push(len_name.clone());
            let view = if info.bytes_view {
                format!("sa_bytes_view({pname}, {len_name})")
            } else {
                format!("sa_utf8({pname}, {len_name})")
            };
            args.push(view);
            i += 2;
            continue;
        }
        let is_out = out_slots
            .map(|(out, cap, len)| i == out || i == cap || i == len)
            .unwrap_or(false);
        if !is_out {
            if sig.params[i].ty == PrimType::BlobHandle {
                args.push(format!("sa_handle_get({pname})"));
            } else {
                args.push(pname);
            }
        }
        i += 1;
    }
    let member = info.member.clone().unwrap_or_default();
    let call = match info.shape {
        NpmBindShape::Method | NpmBindShape::Get => {
            if matches!(info.shape, NpmBindShape::Get) {
                format!("s.{member}")
            } else {
                format!("s.{member}({})", args[1..].join(", "))
            }
        }
        _ => {
            let target = match &info.member {
                Some(_) => format!("{}.{}", info.alias, member),
                None => info.alias.clone(),
            };
            if matches!(info.shape, NpmBindShape::Ctor) {
                format!("new {target}({})", args.join(", "))
            } else {
                format!("{target}({})", args.join(", "))
            }
        }
    };
    let name = js_string(&sig.name);
    buf.push_indent(0, &format!("SA_EXTERNS[{name}] = function ({}) {{", params.join(", ")));
    if matches!(info.shape, NpmBindShape::Method | NpmBindShape::Get) {
        buf.push_indent(1, "const s = sa_handle_get(a0);");
    }
    if let Some((out, cap, len)) = out_slots {
        buf.push_indent(1, &format!("const sa_npm_text = String({call});"));
        buf.push_indent(1, "const sa_npm_bytes = new TextEncoder().encode(sa_npm_text);");
        buf.push_indent(1, &format!("const sa_npm_cap = Number(a{cap});"));
        buf.push_indent(
            1,
            &format!("const sa_npm_n = Math.min(sa_npm_bytes.length, sa_npm_cap);"),
        );
        buf.push_indent(
            1,
            &format!("U8.set(sa_npm_bytes.subarray(0, sa_npm_n), Number(a{out}));"),
        );
        buf.push_indent(
            1,
            &format!("DV.setBigUint64(Number(a{len}), BigInt(sa_npm_bytes.length), true);"),
        );
        buf.push_indent(1, "return sa_npm_bytes.length <= sa_npm_cap ? 0 : 1;");
    } else if sig.return_ty == PrimType::BlobHandle {
        buf.push_indent(1, &format!("return sa_handle_new({call});"));
    } else {
        buf.push_indent(1, &format!("return {call};"));
    }
    buf.push_indent(0, "};");
}
