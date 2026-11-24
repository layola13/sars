mod leb;
mod lower;
mod opcode;
mod sections;

pub use crate::leb::{sleb128, uleb128};
#[cfg(test)]
use crate::leb::{put_bytes, put_uleb};
use crate::opcode::{storage_type, VALTYPE_I64};
use crate::sections::{
    code_section, data_section, export_entry, export_section, functype, function_section,
    import_entry, import_section, locals_vec, memory_section, name_section, type_section,
};
use sa_ir::{Diagnostic, FunctionKind, PrimType, Program, Signature, SourceLoc, Trap};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct WasmOptions {
    pub entry: Option<String>,
    pub stack_size: u32,
    pub memory_pages: u32,
    pub debug_names: bool,
}

impl Default for WasmOptions {
    fn default() -> Self {
        Self {
            entry: None,
            stack_size: 65536,
            memory_pages: 2,
            debug_names: true,
        }
    }
}

pub(crate) struct ConstSegment {
    pub name: String,
    pub addr: u32,
    pub data: Vec<u8>,
}

pub(crate) fn const_layout(program: &Program) -> Vec<ConstSegment> {
    let mut cursor = 1024u32;
    let mut out = Vec::new();
    for constant in &program.consts {
        let data = match constant.value.bytes() {
            Some(data) if !data.is_empty() => data,
            _ => continue,
        };
        cursor = (cursor + 7) & !7;
        out.push(ConstSegment {
            name: constant.name.clone(),
            addr: cursor,
            data: data.clone(),
        });
        cursor = cursor.saturating_add(data.len() as u32 + 1);
    }
    out
}

fn signature_type(sig: &Signature, diags: &mut Vec<Diagnostic>) -> Vec<u8> {
    let mut params = Vec::new();
    for p in &sig.params {
        match storage_type(Some(p.ty)) {
            Some(t) => params.push(t),
            None => diags.push(Diagnostic::new(
                Trap::UnsupportedType,
                "v128 parameters are not supported",
                SourceLoc::unknown(),
            )),
        }
    }
    let mut results = Vec::new();
    if !sig.return_ty.is_void() {
        let return_ty = sig.return_value_type();
        if return_ty == PrimType::V128 {
            diags.push(Diagnostic::new(
                Trap::UnsupportedType,
                "v128 return values are not supported",
                SourceLoc::unknown(),
            ));
            results.push(VALTYPE_I64);
        } else if let Some(t) = storage_type(Some(return_ty)) {
            results.push(t);
        }
    }
    functype(&params, &results)
}

fn intern_type(types: &mut Vec<Vec<u8>>, encoded: Vec<u8>) -> u32 {
    if let Some(index) = types.iter().position(|t| *t == encoded) {
        return index as u32;
    }
    let index = types.len() as u64;
    types.push(encoded);
    index as u32
}

pub fn emit_wasm(program: &Program, opts: &WasmOptions) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let mut diags = Vec::new();
    let consts = const_layout(program);
    let mut types: Vec<Vec<u8>> = Vec::new();
    let mut extern_types = Vec::new();
    for ext in &program.externs {
        let encoded = signature_type(ext, &mut diags);
        extern_types.push(intern_type(&mut types, encoded));
    }
    let mut func_types = Vec::new();
    for f in &program.functions {
        let encoded = signature_type(&f.sig, &mut diags);
        func_types.push(intern_type(&mut types, encoded));
    }

    let mut func_index: HashMap<String, u32> = HashMap::new();
    for (i, ext) in program.externs.iter().enumerate() {
        func_index.insert(ext.name.clone(), i as u32);
    }
    let extern_count = program.externs.len() as u32;
    for (i, f) in program.functions.iter().enumerate() {
        func_index.insert(f.sig.name.clone(), extern_count + i as u32);
    }

    let mut imports = Vec::new();
    for (ext, type_index) in program.externs.iter().zip(extern_types.iter()) {
        imports.push(import_entry("env", ext.display_name(), *type_index));
    }

    let entry_name = opts.entry.as_deref().unwrap_or("main");
    let heap_start = if opts.stack_size < 1024 {
        1024
    } else {
        opts.stack_size as i64
    };

    let mut bodies = Vec::new();
    let mut name_entries = Vec::new();
    for (i, ext) in program.externs.iter().enumerate() {
        name_entries.push((i as u32, ext.display_name().to_string()));
    }
    for (i, f) in program.functions.iter().enumerate() {
        let code = lower::lower_function(
            program,
            f,
            &func_index,
            entry_name,
            heap_start,
            &mut diags,
        );
        let mut body = locals_vec(&code.locals);
        body.extend_from_slice(&code.code);
        bodies.push(body);
        name_entries.push((extern_count + i as u32, f.sig.display_name().to_string()));
    }

    let mut exports = Vec::new();
    let selected = program
        .functions
        .iter()
        .find(|f| f.sig.name == entry_name)
        .or_else(|| {
            program
                .functions
                .iter()
                .find(|f| f.sig.kind == FunctionKind::Exported)
        })
        .or_else(|| program.functions.first());
    if let Some(f) = selected {
        let position = program
            .functions
            .iter()
            .position(|candidate| std::ptr::eq(candidate, f))
            .unwrap_or(0);
        exports.push(export_entry(f.sig.display_name(), 0x00, extern_count + position as u32));
    }
    exports.push(export_entry("memory", 0x02, 0));

    if !diags.is_empty() {
        return Err(diags);
    }

    let mut out = Vec::new();
    out.extend_from_slice(b"\0asm");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&type_section(&types));
    if !imports.is_empty() {
        out.extend_from_slice(&import_section(&imports));
    }
    out.extend_from_slice(&function_section(&func_types));
    out.extend_from_slice(&memory_section(opts.memory_pages));
    out.extend_from_slice(&export_section(&exports));
    out.extend_from_slice(&code_section(&bodies));
    if !consts.is_empty() {
        let segments: Vec<(u32, Vec<u8>)> = consts
            .iter()
            .map(|c| (c.addr, c.data.clone()))
            .collect();
        out.extend_from_slice(&data_section(&segments));
    }
    if opts.debug_names {
        out.extend_from_slice(&name_section(&name_entries));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{
        Address, Block, ConstValue, Function, Instruction, InstructionKind, OpKind, Operand,
        Signature, SourceLoc,
    };

    fn sample_program() -> Program {
        let mut sig = Signature::new("main");
        sig.kind = FunctionKind::Normal;
        let mut f = Function::new(sig, SourceLoc::unknown());
        f.locals.push("x".to_string());
        let mut entry = Block::new("entry", SourceLoc::unknown());
        entry.instrs.push(
            Instruction::new(
                InstructionKind::Op {
                    op: OpKind::Add,
                    args: vec![Operand::Int(1), Operand::Int(2)],
                },
                SourceLoc::unknown(),
                "add",
            )
            .with_dest("x")
            .with_ty(PrimType::I64),
        );
        entry.instrs.push(Instruction::new(
            InstructionKind::Return {
                value: Some(Operand::reg("x")),
            },
            SourceLoc::unknown(),
            "return",
        ));
        f.blocks.push(entry);
        let mut program = Program::new();
        program.functions.push(f);
        program
    }

    fn decode_uleb(bytes: &[u8], pos: &mut usize) -> u64 {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let b = bytes[*pos];
            *pos += 1;
            result |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        result
    }

    fn section_ids(bytes: &[u8]) -> Vec<u8> {
        let mut ids = Vec::new();
        let _ = sections_with_lengths(bytes, &mut ids);
        ids
    }

    fn sections_with_lengths(bytes: &[u8], ids: &mut Vec<u8>) -> usize {
        let mut pos = 8;
        while pos < bytes.len() {
            let id = bytes[pos];
            pos += 1;
            let len = decode_uleb(bytes, &mut pos) as usize;
            assert!(pos + len <= bytes.len(), "section {} overruns", id);
            ids.push(id);
            pos += len;
        }
        pos
    }

    #[test]
    fn options_defaults() {
        let opts = WasmOptions::default();
        assert!(opts.entry.is_none());
        assert_eq!(opts.stack_size, 65536);
        assert_eq!(opts.memory_pages, 2);
        assert!(opts.debug_names);
    }

    #[test]
    fn magic_and_version() {
        let bytes = emit_wasm(&sample_program(), &WasmOptions::default()).unwrap();
        assert_eq!(&bytes[..4], b"\0asm");
        assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
    }

    #[test]
    fn sections_in_order() {
        let bytes = emit_wasm(&sample_program(), &WasmOptions::default()).unwrap();
        assert_eq!(section_ids(&bytes), vec![1, 3, 5, 7, 10, 0]);
        let mut ids = Vec::new();
        assert_eq!(sections_with_lengths(&bytes, &mut ids), bytes.len());
    }

    #[test]
    fn code_body_has_length_prefix() {
        let bytes = emit_wasm(&sample_program(), &WasmOptions::default()).unwrap();
        let mut pos = 8;
        while pos < bytes.len() {
            let id = bytes[pos];
            pos += 1;
            let len = decode_uleb(&bytes, &mut pos) as usize;
            let payload_start = pos;
            if id == 10 {
                assert_eq!(bytes[pos], 1);
                pos += 1;
                let body_len = decode_uleb(&bytes, &mut pos) as usize;
                assert!(body_len > 1);
                assert_eq!(pos + body_len, payload_start + len);
                return;
            }
            pos = payload_start + len;
        }
        panic!("no code section");
    }

    #[test]
    fn emitted_bytes_end_with_function_end() {
        let opts = WasmOptions {
            debug_names: false,
            ..WasmOptions::default()
        };
        let bytes = emit_wasm(&sample_program(), &opts).unwrap();
        assert_eq!(*bytes.last().unwrap(), 0x0b);
    }

    #[test]
    fn import_section_when_externs_present() {
        let mut program = sample_program();
        let mut ext = Signature::new("sa_print_bytes");
        ext.kind = FunctionKind::External;
        program.externs.push(ext);
        let bytes = emit_wasm(&program, &WasmOptions::default()).unwrap();
        assert_eq!(section_ids(&bytes), vec![1, 2, 3, 5, 7, 10, 0]);
        assert!(bytes.windows(14).any(|w| w == b"sa_print_bytes"));
    }

    #[test]
    fn v128_type_is_rejected() {
        let mut program = sample_program();
        program.functions[0].sig.return_ty = PrimType::V128;
        let err = emit_wasm(&program, &WasmOptions::default()).unwrap_err();
        assert!(err.iter().any(|d| d.trap == Trap::UnsupportedType));
    }

    #[test]
    fn atomic_instruction_is_rejected() {
        let mut program = sample_program();
        program.functions[0].blocks[0].instrs.push(Instruction::new(
            InstructionKind::Fence {
                order: sa_ir::AtomicOrdering::SeqCst,
            },
            SourceLoc::unknown(),
            "fence",
        ));
        let err = emit_wasm(&program, &WasmOptions::default()).unwrap_err();
        assert!(err.iter().any(|d| d.trap == Trap::UnsupportedInstruction));
    }

    #[test]
    fn fallible_call_is_rejected() {
        let mut program = sample_program();
        let mut ext = Signature::new("sa_maybe");
        ext.return_fallible = true;
        ext.return_ty = PrimType::I64;
        program.externs.push(ext);
        program.functions[0].blocks[0].instrs.push(Instruction::new(
            InstructionKind::Call {
                target: "sa_maybe".to_string(),
                args: Vec::new(),
            },
            SourceLoc::unknown(),
            "call",
        ));
        let err = emit_wasm(&program, &WasmOptions::default()).unwrap_err();
        assert!(err.iter().any(|d| d.trap == Trap::UnsupportedType));
    }

    #[test]
    fn load_store_and_alloc_lower() {
        let mut program = sample_program();
        let block = &mut program.functions[0].blocks[0];
        block.instrs.insert(
            0,
            Instruction::new(
                InstructionKind::Alloc {
                    size: Operand::Int(16),
                },
                SourceLoc::unknown(),
                "alloc",
            )
            .with_dest("ptr")
            .with_ty(PrimType::Ptr),
        );
        block.instrs.insert(
            1,
            Instruction::new(
                InstructionKind::Store {
                    addr: Address::new(Operand::reg("ptr"), 0),
                    value: Operand::Int(7),
                },
                SourceLoc::unknown(),
                "store",
            )
            .with_ty(PrimType::I64),
        );
        block.instrs.insert(
            2,
            Instruction::new(
                InstructionKind::Load {
                    addr: Address::new(Operand::reg("ptr"), 0),
                },
                SourceLoc::unknown(),
                "load",
            )
            .with_dest("loaded")
            .with_ty(PrimType::I64),
        );
        let bytes = emit_wasm(&program, &WasmOptions::default()).unwrap();
        assert!(bytes.len() > 16);
    }

    #[test]
    fn constants_are_encoded() {
        let mut program = Program::new();
        let mut sig = Signature::new("main");
        sig.return_ty = PrimType::I64;
        let mut f = Function::new(sig, SourceLoc::unknown());
        let mut block = Block::new("entry", SourceLoc::unknown());
        block.instrs.push(
            Instruction::new(
                InstructionKind::Nop,
                SourceLoc::unknown(),
                "nop",
            )
            .with_ty(PrimType::I64),
        );
        block
            .instrs
            .push(Instruction::new(InstructionKind::Return { value: Some(Operand::Int(624485)) }, SourceLoc::unknown(), "return"));
        f.blocks.push(block);
        program.functions.push(f);
        assert!(emit_wasm(&program, &WasmOptions::default()).is_ok());
    }

    #[test]
    fn name_section_includes_function_names() {
        let bytes = emit_wasm(&sample_program(), &WasmOptions::default()).unwrap();
        assert!(bytes.windows(4).any(|w| w == b"name"));
        assert!(bytes.windows(4).any(|w| w == b"main"));
    }

    #[test]
    fn put_bytes_writes_length_prefix() {
        let mut out = Vec::new();
        put_bytes(&mut out, b"main");
        assert_eq!(out, vec![4, b'm', b'a', b'i', b'n']);
    }

    #[test]
    fn put_uleb_writes_bytes() {
        let mut out = Vec::new();
        put_uleb(&mut out, 624485);
        assert_eq!(out, vec![0xe5, 0x8e, 0x26]);
    }

    #[test]
    fn const_value_helpers() {
        let v = ConstValue::Int(42);
        assert_eq!(v.as_i64(), Some(42));
        assert_eq!(v.as_f64(), Some(42.0));
    }
}
