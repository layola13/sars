use crate::{RuntimeError, Trap, Value};
use sa_ir::PrimType;

const MAX_MEMORY: usize = 256 * 1024 * 1024;

pub struct Memory {
    bytes: Vec<u8>,
    brk: usize,
}

impl Memory {
    pub fn new(size: usize) -> Self {
        let size = size.clamp(4096, MAX_MEMORY);
        Self {
            bytes: vec![0; size],
            brk: 1,
        }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn brk(&self) -> usize {
        self.brk
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> Result<u64, RuntimeError> {
        let align = align.max(1).next_power_of_two();
        let start = self.brk.wrapping_add(align - 1) & !(align - 1);
        let end = start
            .checked_add(size)
            .ok_or_else(|| RuntimeError::new(Trap::ArenaOom, "allocation size overflow"))?;
        if end > self.bytes.len() {
            self.grow(end)?;
        }
        if end > self.bytes.len() {
            return Err(RuntimeError::new(Trap::ArenaOom, "arena limit exceeded"));
        }
        self.brk = end;
        Ok(start as u64)
    }

    fn grow(&mut self, needed: usize) -> Result<(), RuntimeError> {
        if needed > MAX_MEMORY {
            return Err(RuntimeError::new(Trap::ArenaOom, "arena limit exceeded"));
        }
        let mut next = self.bytes.len().max(4096);
        while next < needed {
            next = next.saturating_mul(2).min(MAX_MEMORY);
        }
        self.bytes.resize(next, 0);
        Ok(())
    }

    pub fn read(&self, ptr: u64, len: usize) -> Result<&[u8], RuntimeError> {
        let start = ptr as usize;
        let end = start
            .checked_add(len)
            .ok_or_else(|| RuntimeError::new(Trap::RuntimeError, "memory address overflow"))?;
        if end > self.bytes.len() {
            return Err(RuntimeError::new(
                Trap::RuntimeError,
                format!("out of bounds read at {ptr} length {len}"),
            ));
        }
        Ok(&self.bytes[start..end])
    }

    pub fn write(&mut self, ptr: u64, data: &[u8]) -> Result<(), RuntimeError> {
        let start = ptr as usize;
        let end = start
            .checked_add(data.len())
            .ok_or_else(|| RuntimeError::new(Trap::RuntimeError, "memory address overflow"))?;
        if end > self.bytes.len() {
            return Err(RuntimeError::new(
                Trap::RuntimeError,
                format!("out of bounds write at {ptr} length {}", data.len()),
            ));
        }
        self.bytes[start..end].copy_from_slice(data);
        Ok(())
    }

    pub fn load(&self, ptr: u64, ty: PrimType) -> Result<Value, RuntimeError> {
        if ty.is_void() {
            return Ok(Value::Void);
        }
        match ty {
            PrimType::F32 => {
                let b = self.read(ptr, 4)?;
                Ok(Value::Float(f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64))
            }
            PrimType::F64 => {
                let b = self.read(ptr, 8)?;
                let mut raw = [0u8; 8];
                raw.copy_from_slice(b);
                Ok(Value::Float(f64::from_le_bytes(raw)))
            }
            PrimType::Ptr | PrimType::BlobHandle => {
                let b = self.read(ptr, 8)?;
                let mut raw = [0u8; 8];
                raw.copy_from_slice(b);
                Ok(Value::Ptr(u64::from_le_bytes(raw)))
            }
            PrimType::V128 => {
                let b = self.read(ptr, 16)?;
                Ok(Value::Bytes(b.to_vec()))
            }
            _ => {
                let width = ty.bytes() as usize;
                let b = self.read(ptr, width)?;
                let mut raw = [0u8; 16];
                raw[..width].copy_from_slice(b);
                let mut v = u128::from_le_bytes(raw);
                if ty == PrimType::I1 {
                    v &= 1;
                } else if ty.is_signed() {
                    let bits = ty.bits();
                    let sign = 1u128 << (bits - 1);
                    if v & sign != 0 {
                        v |= !((1u128 << bits) - 1);
                    }
                }
                Ok(Value::Int(v as i128))
            }
        }
    }

    pub fn store(&mut self, ptr: u64, ty: PrimType, value: &Value) -> Result<(), RuntimeError> {
        if ty.is_void() {
            return Ok(());
        }
        match ty {
            PrimType::F32 => {
                let raw = (value.as_f64().unwrap_or(0.0) as f32).to_le_bytes();
                self.write(ptr, &raw)
            }
            PrimType::F64 => {
                let raw = value.as_f64().unwrap_or(0.0).to_le_bytes();
                self.write(ptr, &raw)
            }
            PrimType::Ptr | PrimType::BlobHandle => {
                let raw = value.as_u64().unwrap_or(0).to_le_bytes();
                self.write(ptr, &raw)
            }
            PrimType::V128 => match value {
                Value::Bytes(b) => {
                    let mut raw = [0u8; 16];
                    for (i, x) in b.iter().take(16).enumerate() {
                        raw[i] = *x;
                    }
                    self.write(ptr, &raw)
                }
                _ => self.write(ptr, &(value.to_i128() as u128).to_le_bytes()[..16]),
            },
            _ => {
                let width = ty.bytes() as usize;
                let raw = (value.to_i128() as u128).to_le_bytes();
                self.write(ptr, &raw[..width])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_allocator_aligns() {
        let mut m = Memory::new(1024);
        let a = m.alloc(3, 1).unwrap();
        let b = m.alloc(8, 8).unwrap();
        assert!(a >= 1);
        assert_eq!(b % 8, 0);
        assert!(b > a);
    }

    #[test]
    fn load_store_signed_widths() {
        let mut m = Memory::new(1024);
        let p = m.alloc(16, 8).unwrap();
        m.store(p, PrimType::I8, &Value::Int(-3)).unwrap();
        assert_eq!(m.load(p, PrimType::I8).unwrap(), Value::Int(-3));
        assert_eq!(m.load(p, PrimType::U8).unwrap(), Value::Int(253));
        m.store(p, PrimType::F64, &Value::Float(2.5)).unwrap();
        assert_eq!(m.load(p, PrimType::F64).unwrap(), Value::Float(2.5));
        m.store(p, PrimType::Ptr, &Value::Ptr(777)).unwrap();
        assert_eq!(m.load(p, PrimType::Ptr).unwrap(), Value::Ptr(777));
    }

    #[test]
    fn out_of_bounds_is_runtime_error() {
        let m = Memory::new(4096);
        let e = m.read(4095, 4).unwrap_err();
        assert_eq!(e.trap, Trap::RuntimeError);
    }

    #[test]
    fn arena_limit() {
        let mut m = Memory::new(4096);
        let e = m.alloc(MAX_MEMORY, 8).unwrap_err();
        assert_eq!(e.trap, Trap::ArenaOom);
    }
}
