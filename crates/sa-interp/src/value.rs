use crate::{RuntimeError, Trap};
use sa_ir::{OpKind, PrimType};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i128),
    Float(f64),
    Ptr(u64),
    Bytes(Vec<u8>),
    Void,
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => i64::try_from(*v).ok(),
            Self::Float(v) => Some(*v as i64),
            Self::Ptr(v) => Some(*v as i64),
            Self::Bytes(b) => {
                let mut n = 0i64;
                for (i, x) in b.iter().take(8).enumerate() {
                    n |= (*x as i64) << (8 * i);
                }
                Some(n)
            }
            Self::Void => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            Self::Ptr(v) => Some(*v as f64),
            Self::Void | Self::Bytes(_) => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Int(v) => u64::try_from(*v).ok(),
            Self::Float(v) => Some(*v as u64),
            Self::Ptr(v) => Some(*v),
            Self::Bytes(b) => {
                let mut n = 0u64;
                for (i, x) in b.iter().take(8).enumerate() {
                    n |= (*x as u64) << (8 * i);
                }
                Some(n)
            }
            Self::Void => None,
        }
    }

    pub fn to_i128(&self) -> i128 {
        match self {
            Self::Int(v) => *v,
            Self::Float(v) => *v as i128,
            Self::Ptr(v) => *v as i128,
            Self::Bytes(b) => {
                let mut raw = [0u8; 16];
                for (i, x) in b.iter().take(16).enumerate() {
                    raw[i] = *x;
                }
                i128::from_le_bytes(raw)
            }
            Self::Void => 0,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Int(v) => *v != 0,
            Self::Float(v) => *v != 0.0,
            Self::Ptr(v) => *v != 0,
            Self::Bytes(b) => !b.is_empty(),
            Self::Void => false,
        }
    }
}

fn int(v: &Value) -> i128 {
    v.to_i128()
}

fn float(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

fn boolean(b: bool) -> Value {
    Value::Int(if b { 1 } else { 0 })
}

fn bits(ty: Option<PrimType>) -> u32 {
    ty.map(|t| t.bits()).unwrap_or(128).min(128)
}

fn mask(v: i128, ty: Option<PrimType>) -> i128 {
    let n = bits(ty);
    if n == 0 || n >= 128 {
        return v;
    }
    let modulus = 1i128 << n;
    let half = 1i128 << (n - 1);
    let u = v & (modulus - 1);
    let signed = ty.map(|t| t.is_signed()).unwrap_or(true);
    if signed && u & half != 0 {
        u - modulus
    } else {
        u
    }
}

fn sign_extend(v: i128, ty: Option<PrimType>) -> i128 {
    let n = bits(ty);
    if n == 0 || n >= 128 {
        return v;
    }
    let half = 1i128 << (n - 1);
    let u = v & ((1i128 << n) - 1);
    if u & half != 0 {
        u - (1i128 << n)
    } else {
        u
    }
}

fn divide(a: i128, b: i128, unsigned: bool, rem: bool) -> Result<i128, RuntimeError> {
    if b == 0 {
        return Err(RuntimeError::new(Trap::DivisionByZero, "division by zero"));
    }
    if unsigned {
        let (x, y) = (a as u128, b as u128);
        Ok(if rem {
            (x % y) as i128
        } else {
            (x / y) as i128
        })
    } else if rem {
        Ok(a.wrapping_rem(b))
    } else {
        Ok(a.wrapping_div(b))
    }
}

fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Float(y)) => (*x as f64) == *y,
        (Value::Float(x), Value::Int(y)) => *x == (*y as f64),
        _ => a == b,
    }
}

fn lanes(v: &Value) -> [u8; 16] {
    let mut out = [0u8; 16];
    if let Value::Bytes(b) = v {
        for (i, x) in b.iter().take(16).enumerate() {
            out[i] = *x;
        }
    } else {
        let raw = int(v) as u128;
        out.copy_from_slice(&raw.to_le_bytes());
    }
    out
}

fn lane(buf: &[u8; 16], index: usize) -> i32 {
    let i = (index.min(3)) * 4;
    i32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]])
}

fn set_lane(buf: &mut [u8; 16], index: usize, v: i32) {
    let i = (index.min(3)) * 4;
    let raw = v.to_le_bytes();
    buf[i..i + 4].copy_from_slice(&raw);
}

fn vector(op: OpKind, a: &Value, b: &Value) -> Value {
    let mut out = lanes(a);
    let rhs = lanes(b);
    for i in 0..4 {
        let x = lane(&out, i);
        let y = lane(&rhs, i);
        let z = match op {
            OpKind::AddV128 => x.wrapping_add(y),
            OpKind::SubV128 => x.wrapping_sub(y),
            _ => x.wrapping_mul(y),
        };
        set_lane(&mut out, i, z);
    }
    Value::Bytes(out.to_vec())
}

pub(crate) fn apply_op(
    op: OpKind,
    args: &[Value],
    ty: Option<PrimType>,
) -> Result<Value, RuntimeError> {
    let a = args.first().cloned().unwrap_or(Value::Int(0));
    let b = args.get(1).cloned().unwrap_or(Value::Int(0));
    let c = args.get(2).cloned().unwrap_or(Value::Int(0));
    let x = int(&a);
    let y = int(&b);
    match op {
        OpKind::Add => {
            if matches!(a, Value::Float(_)) || matches!(b, Value::Float(_)) {
                Ok(Value::Float(float(&a) + float(&b)))
            } else {
                Ok(Value::Int(mask(x.wrapping_add(y), ty)))
            }
        }
        OpKind::Sub => {
            if matches!(a, Value::Float(_)) || matches!(b, Value::Float(_)) {
                Ok(Value::Float(float(&a) - float(&b)))
            } else {
                Ok(Value::Int(mask(x.wrapping_sub(y), ty)))
            }
        }
        OpKind::Mul => {
            if matches!(a, Value::Float(_)) || matches!(b, Value::Float(_)) {
                Ok(Value::Float(float(&a) * float(&b)))
            } else {
                Ok(Value::Int(mask(x.wrapping_mul(y), ty)))
            }
        }
        OpKind::SDiv | OpKind::Div => Ok(Value::Int(divide(x, y, false, false)?)),
        OpKind::UDiv => Ok(Value::Int(divide(x, y, true, false)?)),
        OpKind::SRem | OpKind::Rem => Ok(Value::Int(divide(x, y, false, true)?)),
        OpKind::URem => Ok(Value::Int(divide(x, y, true, true)?)),
        OpKind::Neg => match a {
            Value::Float(_) => Ok(Value::Float(-float(&a))),
            _ => Ok(Value::Int(mask(x.wrapping_neg(), ty))),
        },
        OpKind::And => Ok(Value::Int(mask(x & y, ty))),
        OpKind::Or => Ok(Value::Int(mask(x | y, ty))),
        OpKind::Xor => Ok(Value::Int(mask(x ^ y, ty))),
        OpKind::Shl => Ok(Value::Int(mask(x.wrapping_shl(y as u32 & 127), ty))),
        OpKind::LShr | OpKind::Shr => Ok(Value::Int(mask(
            (x as u128).wrapping_shr(y as u32 & 127) as i128,
            ty,
        ))),
        OpKind::AShr => Ok(Value::Int(mask(x.wrapping_shr(y as u32 & 127), ty))),
        OpKind::Not => Ok(Value::Int(mask(!x, ty))),
        OpKind::Eq => Ok(boolean(equal(&a, &b))),
        OpKind::Ne => Ok(boolean(!equal(&a, &b))),
        OpKind::Slt | OpKind::Lt => Ok(boolean(x < y)),
        OpKind::Sle => Ok(boolean(x <= y)),
        OpKind::Sgt | OpKind::Gt => Ok(boolean(x > y)),
        OpKind::Sge => Ok(boolean(x >= y)),
        OpKind::Ult => Ok(boolean((x as u128) < (y as u128))),
        OpKind::Ule => Ok(boolean((x as u128) <= (y as u128))),
        OpKind::Ugt => Ok(boolean((x as u128) > (y as u128))),
        OpKind::Uge => Ok(boolean((x as u128) >= (y as u128))),
        OpKind::FAdd => Ok(Value::Float(float(&a) + float(&b))),
        OpKind::FSub => Ok(Value::Float(float(&a) - float(&b))),
        OpKind::FMul => Ok(Value::Float(float(&a) * float(&b))),
        OpKind::FDiv => Ok(Value::Float(float(&a) / float(&b))),
        OpKind::FNeg => Ok(Value::Float(-float(&a))),
        OpKind::FcmpEq => Ok(boolean(float(&a) == float(&b))),
        OpKind::FcmpNe => Ok(boolean(float(&a) != float(&b))),
        OpKind::FcmpLt => Ok(boolean(float(&a) < float(&b))),
        OpKind::FcmpLe => Ok(boolean(float(&a) <= float(&b))),
        OpKind::FcmpGt => Ok(boolean(float(&a) > float(&b))),
        OpKind::FcmpGe => Ok(boolean(float(&a) >= float(&b))),
        OpKind::Trunc | OpKind::Zext => Ok(Value::Int(mask(x, ty))),
        OpKind::Sext => Ok(Value::Int(sign_extend(x, ty))),
        OpKind::Fptosi => Ok(Value::Int(float(&a) as i128)),
        OpKind::Sitofp => Ok(Value::Float(x as f64)),
        OpKind::Uitofp => Ok(Value::Float((x as u128) as f64)),
        OpKind::Fptrunc => Ok(Value::Float((float(&a) as f32) as f64)),
        OpKind::Fpext => Ok(Value::Float(float(&a))),
        OpKind::Bitcast => match a {
            Value::Float(_) => {
                let raw = float(&a).to_bits();
                if ty == Some(PrimType::Ptr) {
                    Ok(Value::Ptr(raw))
                } else {
                    Ok(Value::Int(raw as i128))
                }
            }
            _ => match ty {
                Some(t) if t.is_float() => Ok(Value::Float(f64::from_bits(x as u64))),
                Some(PrimType::Ptr) => Ok(Value::Ptr(x as u64)),
                _ => Ok(Value::Int(x)),
            },
        },
        OpKind::AddV128 | OpKind::SubV128 | OpKind::MulV128 => Ok(vector(op, &a, &b)),
        OpKind::ShuffleV128 => {
            let left = lanes(&a);
            let right = lanes(&b);
            let sel = lanes(&c);
            let mut out = [0u8; 16];
            for i in 0..4 {
                let index = (sel[i] as usize) % 8;
                let source = if index < 4 { &left } else { &right };
                let off = (index % 4) * 4;
                out[i * 4..i * 4 + 4].copy_from_slice(&source[off..off + 4]);
            }
            Ok(Value::Bytes(out.to_vec()))
        }
        OpKind::ExtractLane => {
            let buf = lanes(&a);
            let index = int(&b) as usize;
            Ok(Value::Int(lane(&buf, index) as i128))
        }
        OpKind::InsertLane => {
            let mut buf = lanes(&a);
            let index = int(&b) as usize;
            set_lane(&mut buf, index, int(&c) as i32);
            Ok(Value::Bytes(buf.to_vec()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_helpers() {
        assert_eq!(Value::Int(7).as_i64(), Some(7));
        assert_eq!(Value::Int(-1).as_u64(), None);
        assert_eq!(Value::Ptr(9).as_u64(), Some(9));
        assert_eq!(Value::Int(42).to_i128(), 42);
        assert_eq!(Value::Float(1.5).as_f64(), Some(1.5));
        assert_eq!(Value::Void.as_i64(), None);
        assert!(!Value::Int(0).is_truthy());
        assert!(Value::Bytes(vec![1]).is_truthy());
    }

    #[test]
    fn arithmetic_wraps() {
        let v = apply_op(
            OpKind::Add,
            &[Value::Int(127), Value::Int(1)],
            Some(PrimType::I8),
        )
        .unwrap();
        assert_eq!(v, Value::Int(-128));
        let v = apply_op(OpKind::Mul, &[Value::Int(3), Value::Int(4)], None).unwrap();
        assert_eq!(v, Value::Int(12));
    }

    #[test]
    fn comparisons_and_conversions() {
        assert_eq!(
            apply_op(OpKind::Slt, &[Value::Int(-2), Value::Int(1)], None).unwrap(),
            Value::Int(1)
        );
        assert_eq!(
            apply_op(OpKind::Ult, &[Value::Int(-1), Value::Int(1)], None).unwrap(),
            Value::Int(0)
        );
        assert_eq!(
            apply_op(OpKind::Sitofp, &[Value::Int(3)], None).unwrap(),
            Value::Float(3.0)
        );
        assert_eq!(
            apply_op(OpKind::Fptosi, &[Value::Float(3.9)], None).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn division_by_zero_traps() {
        let e = apply_op(OpKind::SDiv, &[Value::Int(1), Value::Int(0)], None).unwrap_err();
        assert_eq!(e.trap, Trap::DivisionByZero);
    }

    #[test]
    fn vector_ops() {
        let a = Value::Bytes(vec![1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let b = Value::Bytes(vec![3, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let v = apply_op(OpKind::AddV128, &[a.clone(), b], None).unwrap();
        assert_eq!(
            apply_op(OpKind::ExtractLane, &[v, Value::Int(1)], None).unwrap(),
            Value::Int(6)
        );
        let _ = a;
    }
}
