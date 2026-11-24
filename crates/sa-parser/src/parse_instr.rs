use crate::parser::{
    error_at, join, parse_call_args, split_commas, string_value, take_orderings, take_type_suffix,
    top_equal,
};
use sa_ir::{
    Address, AtomicOrdering, AtomicRmwOp, Diagnostic, Instruction, InstructionKind, OpKind,
    Operand, PrimType, SourceLoc, Trap,
};
use sa_lexer::{Token, TokenKind};

pub(crate) fn parse_instruction(
    tokens: &[Token],
    function: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<Instruction> {
    let loc = tokens[0].loc.clone();
    let raw = join(tokens);
    let mut dest: Option<String> = None;
    let mut rhs = tokens;
    if let Some(eq) = top_equal(tokens) {
        if eq == 0 {
            bad(diags, loc.clone(), function, Trap::ParseError, "missing destination");
            return None;
        }
        dest = tokens[..eq]
            .iter()
            .find(|t| t.kind == TokenKind::Ident)
            .map(|t| t.text.clone());
        if dest.is_none() {
            bad(diags, loc.clone(), function, Trap::ParseError, "missing destination");
            return None;
        }
        rhs = &tokens[eq + 1..];
        if rhs.is_empty() {
            bad(diags, loc, function, Trap::ParseError, "missing instruction after '='");
            return None;
        }
    }
    let mut work = rhs.to_vec();
    let ty = take_type_suffix(&mut work);
    let orders = take_orderings(&mut work);
    let order = orders.first().copied().unwrap_or(AtomicOrdering::Relaxed);
    if work.is_empty() {
        bad(diags, loc, function, Trap::ParseError, "empty instruction");
        return None;
    }
    let first = &work[0];
    let kind = if first.is_ident("alloc") {
        InstructionKind::Alloc {
            size: Operand::parse(&join(&work[1..])),
        }
    } else if first.is_ident("stack_alloc") {
        InstructionKind::StackAlloc {
            size: Operand::parse(&join(&work[1..])),
        }
    } else if first.is_ident("load") {
        InstructionKind::Load {
            addr: Address::parse(&join(&work[1..])),
        }
    } else if first.is_ident("store") {
        let parts = split_commas(&work[1..]);
        if parts.len() < 2 {
            return missing_operand(diags, loc, function, "store");
        }
        InstructionKind::Store {
            addr: Address::parse(&join(&parts[0])),
            value: Operand::parse(&join(&parts[1])),
        }
    } else if first.is_ident("jmp") {
        InstructionKind::Jmp {
            target: join(&work[1..]),
        }
    } else if first.is_ident("br") {
        let Some(arrow) = work.iter().position(|t| t.kind == TokenKind::Arrow) else {
            return missing_operand(diags, loc, function, "br");
        };
        let parts = split_commas(&work[arrow + 1..]);
        if parts.len() < 2 {
            return missing_operand(diags, loc, function, "br");
        }
        InstructionKind::Br {
            cond: Operand::parse(&join(&work[1..arrow])),
            then_label: join(&parts[0]),
            else_label: join(&parts[1]),
        }
    } else if first.is_ident("br_null") {
        let parts = split_commas(&work[1..]);
        if parts.len() < 2 {
            return missing_operand(diags, loc, function, "br_null");
        }
        InstructionKind::BrNull {
            cond: Operand::parse(&join(&parts[0])),
            target: join(&parts[1]),
        }
    } else if first.is_ident("call") {
        let lp = work.iter().position(|t| t.kind == TokenKind::LParen);
        let rp = work.iter().rposition(|t| t.kind == TokenKind::RParen);
        let target = work.get(1).filter(|t| t.kind == TokenKind::Directive);
        match (target, lp, rp) {
            (Some(t), Some(lp), Some(rp)) if lp >= 2 && rp > lp => InstructionKind::Call {
                target: t.text.trim_start_matches('@').to_string(),
                args: parse_call_args(&work[lp + 1..rp]),
            },
            _ => return missing_operand(diags, loc, function, "call"),
        }
    } else if first.is_ident("call_indirect") {
        let lp = work.iter().position(|t| t.kind == TokenKind::LParen);
        let rp = work.iter().rposition(|t| t.kind == TokenKind::RParen);
        match (lp, rp) {
            (Some(lp), Some(rp)) if rp > lp => InstructionKind::CallIndirect {
                target: Operand::parse(&join(&work[1..lp])),
                args: parse_call_args(&work[lp + 1..rp]),
            },
            _ => return missing_operand(diags, loc, function, "call_indirect"),
        }
    } else if first.is_ident("atomic_load") {
        InstructionKind::AtomicLoad {
            addr: Address::parse(&join(&work[1..])),
            order,
        }
    } else if first.is_ident("atomic_store") {
        let parts = split_commas(&work[1..]);
        if parts.len() < 2 {
            return missing_operand(diags, loc, function, "atomic_store");
        }
        InstructionKind::AtomicStore {
            addr: Address::parse(&join(&parts[0])),
            value: Operand::parse(&join(&parts[1])),
            order,
        }
    } else if first.is_ident("cmpxchg") {
        let parts = split_commas(&work[1..]);
        if parts.len() < 3 {
            return missing_operand(diags, loc, function, "cmpxchg");
        }
        InstructionKind::Cmpxchg {
            addr: Address::parse(&join(&parts[0])),
            expected: Operand::parse(&join(&parts[1])),
            replacement: Operand::parse(&join(&parts[2])),
            order,
        }
    } else if first.kind == TokenKind::Ident && first.text.starts_with("atomic_rmw_") {
        let op = AtomicRmwOp::parse(&first.text["atomic_rmw_".len()..]);
        let Some(op) = op else {
            bad(
                diags,
                loc,
                function,
                Trap::UnsupportedInstruction,
                &format!("unknown atomic operation {}", first.text),
            );
            return None;
        };
        let parts = split_commas(&work[1..]);
        if parts.len() < 2 {
            return missing_operand(diags, loc, function, "atomic_rmw");
        }
        InstructionKind::AtomicRmw {
            op,
            addr: Address::parse(&join(&parts[0])),
            value: Operand::parse(&join(&parts[1])),
            order,
        }
    } else if first.is_ident("atomic_rmw") {
        let op = work.get(1).and_then(|t| AtomicRmwOp::parse(&t.text));
        let Some(op) = op else {
            return missing_operand(diags, loc, function, "atomic_rmw");
        };
        let parts = split_commas(&work[2..]);
        if parts.len() < 2 {
            return missing_operand(diags, loc, function, "atomic_rmw");
        }
        InstructionKind::AtomicRmw {
            op,
            addr: Address::parse(&join(&parts[0])),
            value: Operand::parse(&join(&parts[1])),
            order,
        }
    } else if first.is_ident("fence") {
        InstructionKind::Fence { order }
    } else if first.kind == TokenKind::Amp {
        let (mutable, start) = if work.get(1).map(|t| t.is_ident("mut")).unwrap_or(false) {
            (true, 2)
        } else {
            (false, 1)
        };
        InstructionKind::Borrow {
            src: Operand::parse(&join(&work[start..])),
            mutable,
        }
    } else if first.kind == TokenKind::Caret {
        InstructionKind::Move {
            src: Operand::parse(&join(&work[1..])),
        }
    } else if first.kind == TokenKind::Star {
        InstructionKind::RawCast {
            src: Operand::parse(&join(&work[1..])),
        }
    } else if first.kind == TokenKind::Question {
        InstructionKind::Try {
            src: Operand::parse(&join(&work[1..])),
        }
    } else if first.is_ident("assume_safe") {
        InstructionKind::AssumeSafe {
            src: Operand::parse(&join(&work[1..])),
        }
    } else if first.is_ident("assume_borrow") {
        let parts = split_commas(&work[1..]);
        let mutable = parts
            .get(1)
            .map(|p| p.len() == 1 && p[0].is_ident("mut"))
            .unwrap_or(false);
        InstructionKind::AssumeBorrow {
            src: Operand::parse(&join(&parts[0])),
            mutable,
        }
    } else if first.is_ident("ptr_add") {
        let parts = split_commas(&work[1..]);
        if parts.len() < 2 {
            return missing_operand(diags, loc, function, "ptr_add");
        }
        InstructionKind::PtrAdd {
            base: Operand::parse(&join(&parts[0])),
            offset: Operand::parse(&join(&parts[1])),
        }
    } else if first.is_ident("return") {
        InstructionKind::Return {
            value: optional_operand(&work[1..]),
        }
    } else if first.is_ident("early_return") {
        InstructionKind::EarlyReturn {
            value: optional_operand(&work[1..]),
        }
    } else if first.is_ident("panic") || first.is_ident("panic_msg") {
        let lp = work.iter().position(|t| t.kind == TokenKind::LParen);
        let rp = work.iter().rposition(|t| t.kind == TokenKind::RParen);
        match (lp, rp) {
            (Some(lp), Some(rp)) if rp > lp => {
                let parts = split_commas(&work[lp + 1..rp]);
                let has_message = first.is_ident("panic")
                    && parts.first().map(|p| p.first().map(|t| t.kind)) == Some(Some(TokenKind::Str));
                let message = if has_message {
                    parts.first().and_then(|p| p.first()).map(|t| string_value(&t.text))
                } else {
                    None
                };
                let skip = usize::from(has_message);
                InstructionKind::Panic {
                    message,
                    args: parts
                        .iter()
                        .skip(skip)
                        .map(|p| Operand::parse(&join(p)))
                        .collect(),
                }
            }
            _ => return missing_operand(diags, loc, function, "panic"),
        }
    } else if first.kind == TokenKind::Bang {
        InstructionKind::Release {
            reg: join(&work[1..]),
        }
    } else if first.is_ident("take") {
        InstructionKind::Take {
            reg: join(&work[1..]),
        }
    } else if first.is_ident("va_start") {
        InstructionKind::VaStart
    } else if first.is_ident("va_arg") {
        InstructionKind::VaArg {
            src: Operand::parse(&join(&work[1..])),
        }
    } else if first.is_ident("va_end") {
        InstructionKind::VaEnd
    } else if first.is_ident("nop") {
        InstructionKind::Nop
    } else if first.kind == TokenKind::Ident {
        if let Some(op) = OpKind::parse(&first.text) {
            InstructionKind::Op {
                op,
                args: split_commas(&work[1..])
                    .iter()
                    .map(|p| Operand::parse(&join(p)))
                    .collect(),
            }
        } else if dest.is_some() {
            InstructionKind::Assign {
                src: Operand::parse(&join(&work)),
            }
        } else {
            bad(
                diags,
                loc,
                function,
                Trap::UnsupportedInstruction,
                &format!("unsupported instruction {}", first.text),
            );
            return None;
        }
    } else if dest.is_some() {
        InstructionKind::Assign {
            src: Operand::parse(&join(&work)),
        }
    } else {
        bad(
            diags,
            loc,
            function,
            Trap::UnsupportedInstruction,
            &format!("unsupported instruction {}", join(&work)),
        );
        return None;
    };
    Some(finish(
        kind,
        dest.as_deref(),
        ty,
        loc,
        raw,
    ))
}

fn finish(
    kind: InstructionKind,
    dest: Option<&str>,
    ty: Option<PrimType>,
    loc: SourceLoc,
    raw: String,
) -> Instruction {
    let mut ins = Instruction::new(kind, loc, raw);
    if let Some(d) = dest {
        ins = ins.with_dest(d);
    }
    if let Some(t) = ty {
        ins = ins.with_ty(t);
    }
    ins
}

fn optional_operand(tokens: &[Token]) -> Option<Operand> {
    if tokens.is_empty() {
        None
    } else {
        Some(Operand::parse(&join(tokens)))
    }
}

fn missing_operand(
    diags: &mut Vec<Diagnostic>,
    loc: SourceLoc,
    function: &str,
    name: &str,
) -> Option<Instruction> {
    bad(
        diags,
        loc,
        function,
        Trap::ParseError,
        &format!("invalid or missing operands for {name}"),
    );
    None
}

fn bad(
    diags: &mut Vec<Diagnostic>,
    loc: SourceLoc,
    function: &str,
    trap: Trap,
    message: &str,
) {
    diags.push(error_at(loc, trap, message).in_function(function));
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{CallArg, CapPrefix};

    fn parse(text: &str) -> Instruction {
        let toks = sa_lexer::tokenize(text, "t.sa").unwrap();
        let line: Vec<Token> = toks.into_iter().filter(|t| t.kind != TokenKind::Eof).collect();
        let mut diags = Vec::new();
        let ins = parse_instruction(&line, "f", &mut diags).unwrap();
        assert!(diags.is_empty(), "{text}: {diags:?}");
        ins
    }

    #[test]
    fn memory_instructions() {
        assert_eq!(
            parse("x = alloc 16").kind,
            InstructionKind::Alloc { size: Operand::Int(16) }
        );
        assert_eq!(
            parse("x = stack_alloc 8").kind,
            InstructionKind::StackAlloc { size: Operand::Int(8) }
        );
        let load = parse("x = load arr+4 as i32");
        assert_eq!(
            load.kind,
            InstructionKind::Load { addr: Address::new(Operand::reg("arr"), 4) }
        );
        assert_eq!(load.ty, Some(PrimType::I32));
        assert_eq!(load.dest.as_deref(), Some("x"));
        let store = parse("store arr+0, 7 as i64");
        assert_eq!(
            store.kind,
            InstructionKind::Store {
                addr: Address::new(Operand::reg("arr"), 0),
                value: Operand::Int(7),
            }
        );
        assert_eq!(store.ty, Some(PrimType::I64));
    }

    #[test]
    fn ops_and_assignments() {
        let op = parse("sum = add a, 3");
        assert_eq!(
            op.kind,
            InstructionKind::Op { op: OpKind::Add, args: vec![Operand::reg("a"), Operand::Int(3)] }
        );
        assert_eq!(op.dest.as_deref(), Some("sum"));
        let cast = parse("one = 1 as i32");
        assert_eq!(cast.kind, InstructionKind::Assign { src: Operand::Int(1) });
        assert_eq!(cast.ty, Some(PrimType::I32));
        assert_eq!(
            parse("neg = fneg x as f64").kind,
            InstructionKind::Op { op: OpKind::FNeg, args: vec![Operand::reg("x")] }
        );
    }

    #[test]
    fn calls_with_capabilities() {
        let call = parse("call @print(&OUT, ^tmp, *ptr, 3)");
        assert_eq!(
            call.kind,
            InstructionKind::Call {
                target: "print".into(),
                args: vec![
                    CallArg::new(CapPrefix::Borrow, Operand::reg("OUT")),
                    CallArg::new(CapPrefix::Move, Operand::reg("tmp")),
                    CallArg::new(CapPrefix::Raw, Operand::reg("ptr")),
                    CallArg::new(CapPrefix::ByValue, Operand::Int(3)),
                ],
            }
        );
        let with_dest = parse("r = call @halfstep()");
        assert_eq!(with_dest.dest.as_deref(), Some("r"));
        assert!(matches!(with_dest.kind, InstructionKind::Call { .. }));
        let indirect = parse("f = call_indirect fp(&x)");
        assert_eq!(
            indirect.kind,
            InstructionKind::CallIndirect {
                target: Operand::reg("fp"),
                args: vec![CallArg::new(CapPrefix::Borrow, Operand::reg("x"))],
            }
        );
    }

    #[test]
    fn control_flow() {
        assert_eq!(
            parse("jmp L_LOOP").kind,
            InstructionKind::Jmp { target: "L_LOOP".into() }
        );
        assert_eq!(
            parse("br c -> L_A, L_B").kind,
            InstructionKind::Br {
                cond: Operand::reg("c"),
                then_label: "L_A".into(),
                else_label: "L_B".into(),
            }
        );
        assert_eq!(
            parse("br_null p, L_NULL").kind,
            InstructionKind::BrNull { cond: Operand::reg("p"), target: "L_NULL".into() }
        );
        assert_eq!(parse("return").kind, InstructionKind::Return { value: None });
        assert_eq!(
            parse("early_return 4").kind,
            InstructionKind::EarlyReturn { value: Some(Operand::Int(4)) }
        );
    }

    #[test]
    fn capability_instructions() {
        assert_eq!(
            parse("r = &x").kind,
            InstructionKind::Borrow { src: Operand::reg("x"), mutable: false }
        );
        assert_eq!(
            parse("r = &mut x").kind,
            InstructionKind::Borrow { src: Operand::reg("x"), mutable: true }
        );
        assert_eq!(parse("r = ^x").kind, InstructionKind::Move { src: Operand::reg("x") });
        assert_eq!(parse("r = *x").kind, InstructionKind::RawCast { src: Operand::reg("x") });
        assert_eq!(parse("r = ?x").kind, InstructionKind::Try { src: Operand::reg("x") });
        assert_eq!(parse("assume_safe x").kind, InstructionKind::AssumeSafe { src: Operand::reg("x") });
        assert_eq!(
            parse("assume_borrow x, mut").kind,
            InstructionKind::AssumeBorrow { src: Operand::reg("x"), mutable: true }
        );
        assert_eq!(parse("!x").kind, InstructionKind::Release { reg: "x".into() });
        assert_eq!(parse("take x").kind, InstructionKind::Take { reg: "x".into() });
    }

    #[test]
    fn atomics_and_misc() {
        assert_eq!(
            parse("x = atomic_load p+0 as i32 acquire").kind,
            InstructionKind::AtomicLoad {
                addr: Address::new(Operand::reg("p"), 0),
                order: AtomicOrdering::Acquire,
            }
        );
        assert_eq!(
            parse("atomic_store p+0, 1 as i32 release").kind,
            InstructionKind::AtomicStore {
                addr: Address::new(Operand::reg("p"), 0),
                value: Operand::Int(1),
                order: AtomicOrdering::Release,
            }
        );
        assert_eq!(
            parse("old, ok = cmpxchg p+0, 0, 1 as i32 acquire").kind,
            InstructionKind::Cmpxchg {
                addr: Address::new(Operand::reg("p"), 0),
                expected: Operand::Int(0),
                replacement: Operand::Int(1),
                order: AtomicOrdering::Acquire,
            }
        );
        assert!(matches!(
            parse("old = atomic_rmw_add p+0, 3 seq_cst").kind,
            InstructionKind::AtomicRmw { op: AtomicRmwOp::Add, order: AtomicOrdering::SeqCst, .. }
        ));
        assert_eq!(parse("fence").kind, InstructionKind::Fence { order: AtomicOrdering::Relaxed });
        assert_eq!(
            parse("panic(\"oops\")").kind,
            InstructionKind::Panic { message: Some("oops".into()), args: vec![] }
        );
        assert_eq!(parse("va_start").kind, InstructionKind::VaStart);
        assert_eq!(parse("nop").kind, InstructionKind::Nop);
    }

    #[test]
    fn unsupported_lines_report_diagnostics() {
        let toks = sa_lexer::tokenize("frobnicate x", "t.sa").unwrap();
        let line: Vec<Token> = toks.into_iter().filter(|t| t.kind != TokenKind::Eof).collect();
        let mut diags = Vec::new();
        assert!(parse_instruction(&line, "f", &mut diags).is_none());
        assert_eq!(diags[0].trap, Trap::UnsupportedInstruction);
    }
}
