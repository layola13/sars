#![allow(dead_code)]

use sa_ir::PrimType;

pub const VALTYPE_I32: u8 = 0x7f;
pub const VALTYPE_I64: u8 = 0x7e;
pub const VALTYPE_F32: u8 = 0x7d;
pub const VALTYPE_F64: u8 = 0x7c;
pub const VALTYPE_V128: u8 = 0x7b;

pub const UNREACHABLE: u8 = 0x00;
pub const NOP: u8 = 0x01;
pub const BLOCK: u8 = 0x02;
pub const LOOP: u8 = 0x03;
pub const IF: u8 = 0x04;
pub const ELSE: u8 = 0x05;
pub const END: u8 = 0x0b;
pub const BR: u8 = 0x0c;
pub const BR_IF: u8 = 0x0d;
pub const BR_TABLE: u8 = 0x0e;
pub const RETURN: u8 = 0x0f;
pub const CALL: u8 = 0x10;
pub const CALL_INDIRECT: u8 = 0x11;
pub const DROP: u8 = 0x1a;
pub const SELECT: u8 = 0x1b;
pub const LOCAL_GET: u8 = 0x20;
pub const LOCAL_SET: u8 = 0x21;
pub const LOCAL_TEE: u8 = 0x22;
pub const GLOBAL_GET: u8 = 0x23;
pub const GLOBAL_SET: u8 = 0x24;

pub const I32_LOAD: u8 = 0x28;
pub const I64_LOAD: u8 = 0x29;
pub const F32_LOAD: u8 = 0x2a;
pub const F64_LOAD: u8 = 0x2b;
pub const I32_LOAD8_S: u8 = 0x2c;
pub const I32_LOAD8_U: u8 = 0x2d;
pub const I32_LOAD16_S: u8 = 0x2e;
pub const I32_LOAD16_U: u8 = 0x2f;
pub const I64_LOAD8_S: u8 = 0x30;
pub const I64_LOAD8_U: u8 = 0x31;
pub const I64_LOAD16_S: u8 = 0x32;
pub const I64_LOAD16_U: u8 = 0x33;
pub const I64_LOAD32_S: u8 = 0x34;
pub const I64_LOAD32_U: u8 = 0x35;
pub const I32_STORE: u8 = 0x36;
pub const I64_STORE: u8 = 0x37;
pub const F32_STORE: u8 = 0x38;
pub const F64_STORE: u8 = 0x39;
pub const I32_STORE8: u8 = 0x3a;
pub const I32_STORE16: u8 = 0x3b;
pub const I64_STORE8: u8 = 0x3c;
pub const I64_STORE16: u8 = 0x3d;
pub const I64_STORE32: u8 = 0x3e;
pub const MEMORY_SIZE: u8 = 0x3f;
pub const MEMORY_GROW: u8 = 0x40;

pub const I32_CONST: u8 = 0x41;
pub const I64_CONST: u8 = 0x42;
pub const F32_CONST: u8 = 0x43;
pub const F64_CONST: u8 = 0x44;

pub const I32_EQZ: u8 = 0x45;
pub const I32_EQ: u8 = 0x46;
pub const I32_NE: u8 = 0x47;
pub const I32_LT_S: u8 = 0x48;
pub const I32_LT_U: u8 = 0x49;
pub const I32_GT_S: u8 = 0x4a;
pub const I32_GT_U: u8 = 0x4b;
pub const I32_LE_S: u8 = 0x4c;
pub const I32_LE_U: u8 = 0x4d;
pub const I32_GE_S: u8 = 0x4e;
pub const I32_GE_U: u8 = 0x4f;
pub const I64_EQZ: u8 = 0x50;
pub const I64_EQ: u8 = 0x51;
pub const I64_NE: u8 = 0x52;
pub const I64_LT_S: u8 = 0x53;
pub const I64_LT_U: u8 = 0x54;
pub const I64_GT_S: u8 = 0x55;
pub const I64_GT_U: u8 = 0x56;
pub const I64_LE_S: u8 = 0x57;
pub const I64_LE_U: u8 = 0x58;
pub const I64_GE_S: u8 = 0x59;
pub const I64_GE_U: u8 = 0x5a;
pub const F32_EQ: u8 = 0x5b;
pub const F32_NE: u8 = 0x5c;
pub const F32_LT: u8 = 0x5d;
pub const F32_GT: u8 = 0x5e;
pub const F32_LE: u8 = 0x5f;
pub const F32_GE: u8 = 0x60;
pub const F64_EQ: u8 = 0x61;
pub const F64_NE: u8 = 0x62;
pub const F64_LT: u8 = 0x63;
pub const F64_GT: u8 = 0x64;
pub const F64_LE: u8 = 0x65;
pub const F64_GE: u8 = 0x66;

pub const I32_ADD: u8 = 0x6a;
pub const I32_SUB: u8 = 0x6b;
pub const I32_MUL: u8 = 0x6c;
pub const I64_ADD: u8 = 0x7c;
pub const I64_SUB: u8 = 0x7d;
pub const I64_MUL: u8 = 0x7e;
pub const I64_DIV_S: u8 = 0x7f;
pub const I64_DIV_U: u8 = 0x80;
pub const I64_REM_S: u8 = 0x81;
pub const I64_REM_U: u8 = 0x82;
pub const I64_AND: u8 = 0x83;
pub const I64_OR: u8 = 0x84;
pub const I64_XOR: u8 = 0x85;
pub const I64_SHL: u8 = 0x86;
pub const I64_SHR_S: u8 = 0x87;
pub const I64_SHR_U: u8 = 0x88;

pub const F64_NEG: u8 = 0x9a;
pub const F64_ADD: u8 = 0xa0;
pub const F64_SUB: u8 = 0xa1;
pub const F64_MUL: u8 = 0xa2;
pub const F64_DIV: u8 = 0xa3;

pub const I32_WRAP_I64: u8 = 0xa7;
pub const I32_TRUNC_F64_S: u8 = 0xaa;
pub const I32_TRUNC_F64_U: u8 = 0xab;
pub const I64_EXTEND_I32_S: u8 = 0xac;
pub const I64_EXTEND_I32_U: u8 = 0xad;
pub const I64_TRUNC_F64_S: u8 = 0xb0;
pub const I64_TRUNC_F64_U: u8 = 0xb1;
pub const F64_CONVERT_I64_S: u8 = 0xb9;
pub const F64_CONVERT_I64_U: u8 = 0xba;
pub const F64_PROMOTE_F32: u8 = 0xbb;
pub const F32_DEMOTE_F64: u8 = 0xb6;

pub fn valtype_for_prim(ty: PrimType) -> Option<u8> {
    match ty {
        PrimType::I1 | PrimType::I8 | PrimType::I16 | PrimType::I32 | PrimType::U8
        | PrimType::U16 | PrimType::U32 => Some(VALTYPE_I32),
        PrimType::I64 | PrimType::U64 | PrimType::Ptr | PrimType::BlobHandle => {
            Some(VALTYPE_I64)
        }
        PrimType::F32 => Some(VALTYPE_F32),
        PrimType::F64 => Some(VALTYPE_F64),
        PrimType::V128 => None,
        PrimType::Void => None,
    }
}

pub fn storage_type(ty: Option<PrimType>) -> Option<u8> {
    match ty {
        Some(PrimType::V128) => None,
        Some(PrimType::F32) | Some(PrimType::F64) => Some(VALTYPE_F64),
        _ => Some(VALTYPE_I64),
    }
}

pub fn is_signed_prim(ty: Option<PrimType>) -> bool {
    match ty {
        Some(t) => t.is_signed(),
        None => true,
    }
}

pub fn load_opcode(ty: Option<PrimType>) -> Option<(u8, u32)> {
    let v = match valtype_for_prim(ty.unwrap_or(PrimType::I64)) {
        Some(v) => v,
        None => return None,
    };
    Some(match v {
        VALTYPE_I32 => match ty {
            Some(PrimType::I1) => (I32_LOAD8_U, 0),
            Some(PrimType::I8) => (I32_LOAD8_S, 0),
            Some(PrimType::U8) => (I32_LOAD8_U, 0),
            Some(PrimType::I16) => (I32_LOAD16_S, 1),
            Some(PrimType::U16) => (I32_LOAD16_U, 1),
            _ => (I32_LOAD, 2),
        },
        VALTYPE_F32 => (F32_LOAD, 2),
        VALTYPE_F64 => (F64_LOAD, 3),
        _ => (I64_LOAD, 3),
    })
}

pub fn store_opcode(ty: Option<PrimType>) -> Option<(u8, u32)> {
    let v = match valtype_for_prim(ty.unwrap_or(PrimType::I64)) {
        Some(v) => v,
        None => return None,
    };
    Some(match v {
        VALTYPE_I32 => match ty {
            Some(PrimType::I1) | Some(PrimType::I8) | Some(PrimType::U8) => (I32_STORE8, 0),
            Some(PrimType::I16) | Some(PrimType::U16) => (I32_STORE16, 1),
            _ => (I32_STORE, 2),
        },
        VALTYPE_F32 => (F32_STORE, 2),
        VALTYPE_F64 => (F64_STORE, 3),
        _ => (I64_STORE, 3),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prim_valtypes() {
        assert_eq!(valtype_for_prim(PrimType::I1), Some(VALTYPE_I32));
        assert_eq!(valtype_for_prim(PrimType::U32), Some(VALTYPE_I32));
        assert_eq!(valtype_for_prim(PrimType::I64), Some(VALTYPE_I64));
        assert_eq!(valtype_for_prim(PrimType::Ptr), Some(VALTYPE_I64));
        assert_eq!(valtype_for_prim(PrimType::BlobHandle), Some(VALTYPE_I64));
        assert_eq!(valtype_for_prim(PrimType::F32), Some(VALTYPE_F32));
        assert_eq!(valtype_for_prim(PrimType::F64), Some(VALTYPE_F64));
        assert_eq!(valtype_for_prim(PrimType::V128), None);
    }

    #[test]
    fn storage_types() {
        assert_eq!(storage_type(None), Some(VALTYPE_I64));
        assert_eq!(storage_type(Some(PrimType::I32)), Some(VALTYPE_I64));
        assert_eq!(storage_type(Some(PrimType::F32)), Some(VALTYPE_F64));
        assert_eq!(storage_type(Some(PrimType::V128)), None);
    }

    #[test]
    fn memory_opcodes() {
        assert_eq!(load_opcode(Some(PrimType::I32)), Some((I32_LOAD, 2)));
        assert_eq!(load_opcode(Some(PrimType::F64)), Some((F64_LOAD, 3)));
        assert_eq!(store_opcode(Some(PrimType::U8)), Some((I32_STORE8, 0)));
        assert_eq!(load_opcode(Some(PrimType::V128)), None);
    }
}
