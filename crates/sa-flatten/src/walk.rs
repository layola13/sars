use sa_ir::{InstructionKind, Operand};

pub(crate) fn rewrite_operands<F>(kind: &mut InstructionKind, f: &mut F)
where
    F: FnMut(&Operand) -> Operand,
{
    match kind {
        InstructionKind::Alloc { size } | InstructionKind::StackAlloc { size } => {
            let next = f(size);
            *size = next;
        }
        InstructionKind::Load { addr } => {
            let next = f(&addr.base);
            addr.base = next;
        }
        InstructionKind::Store { addr, value } => {
            let next = f(&addr.base);
            addr.base = next;
            let next = f(value);
            *value = next;
        }
        InstructionKind::Op { args, .. } => {
            for arg in args.iter_mut() {
                let next = f(arg);
                *arg = next;
            }
        }
        InstructionKind::Br { cond, .. } | InstructionKind::BrNull { cond, .. } => {
            let next = f(cond);
            *cond = next;
        }
        InstructionKind::Call { args, .. } | InstructionKind::CallIndirect { args, .. } => {
            for arg in args.iter_mut() {
                let next = f(&arg.value);
                arg.value = next;
            }
        }
        InstructionKind::Return { value } | InstructionKind::EarlyReturn { value } => {
            if let Some(value) = value {
                let next = f(value);
                *value = next;
            }
        }
        InstructionKind::Assign { src }
        | InstructionKind::Move { src }
        | InstructionKind::RawCast { src }
        | InstructionKind::AssumeSafe { src }
        | InstructionKind::Try { src }
        | InstructionKind::VaArg { src } => {
            let next = f(src);
            *src = next;
        }
        InstructionKind::PtrAdd { base, offset } => {
            let next = f(base);
            *base = next;
            let next = f(offset);
            *offset = next;
        }
        InstructionKind::Borrow { src, .. } | InstructionKind::AssumeBorrow { src, .. } => {
            let next = f(src);
            *src = next;
        }
        InstructionKind::AtomicLoad { addr, .. } => {
            let next = f(&addr.base);
            addr.base = next;
        }
        InstructionKind::AtomicStore { addr, value, .. } => {
            let next = f(&addr.base);
            addr.base = next;
            let next = f(value);
            *value = next;
        }
        InstructionKind::Cmpxchg {
            addr,
            expected,
            replacement,
            ..
        } => {
            let next = f(&addr.base);
            addr.base = next;
            let next = f(expected);
            *expected = next;
            let next = f(replacement);
            *replacement = next;
        }
        InstructionKind::AtomicRmw { addr, value, .. } => {
            let next = f(&addr.base);
            addr.base = next;
            let next = f(value);
            *value = next;
        }
        InstructionKind::Panic { args, .. } => {
            for arg in args.iter_mut() {
                let next = f(arg);
                *arg = next;
            }
        }
        InstructionKind::Take { .. }
        | InstructionKind::Release { .. }
        | InstructionKind::Jmp { .. }
        | InstructionKind::Fence { .. }
        | InstructionKind::VaStart
        | InstructionKind::VaEnd
        | InstructionKind::Native { .. }
        | InstructionKind::Nop => {}
        InstructionKind::Transfer { container, value } => {
            let next = f(container);
            *container = next;
            let next = f(value);
            *value = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{Address, Instruction, InstructionKind, PrimType, SourceLoc};

    fn ins(kind: InstructionKind) -> Instruction {
        Instruction::new(kind, SourceLoc::unknown(), "test")
    }

    #[test]
    fn rewrites_nested_address_and_call_operands() {
        let mut instruction = ins(InstructionKind::Store {
            addr: Address::new(Operand::reg("base"), 0),
            value: Operand::reg("val"),
        });
        rewrite_operands(&mut instruction.kind, &mut |op| match op {
            Operand::Reg(name) if name == "base" => Operand::Int(7),
            other => other.clone(),
        });
        match instruction.kind {
            InstructionKind::Store { addr, value } => {
                assert_eq!(addr.base, Operand::Int(7));
                assert_eq!(value, Operand::reg("val"));
            }
            _ => panic!("wrong kind"),
        }

        let mut call = ins(InstructionKind::Call {
            target: "f".into(),
            args: vec![sa_ir::CallArg::new(
                sa_ir::CapPrefix::ByValue,
                Operand::reg("x"),
            )],
        });
        rewrite_operands(&mut call.kind, &mut |op| match op {
            Operand::Reg(name) if name == "x" => Operand::Int(1),
            other => other.clone(),
        });
        match call.kind {
            InstructionKind::Call { args, .. } => assert_eq!(args[0].value, Operand::Int(1)),
            _ => panic!("wrong kind"),
        }
    }

    #[test]
    fn rewrites_typed_instruction() {
        let mut instruction = Instruction::new(
            InstructionKind::Load {
                addr: Address::new(Operand::reg("p"), 4),
            },
            SourceLoc::unknown(),
            "load p+4",
        )
        .with_ty(PrimType::I64);
        rewrite_operands(&mut instruction.kind, &mut |_op| Operand::Int(9));
        assert!(matches!(
            instruction.kind,
            InstructionKind::Load {
                addr: Address {
                    base: Operand::Int(9),
                    offset: 4
                }
            }
        ));
    }
}
