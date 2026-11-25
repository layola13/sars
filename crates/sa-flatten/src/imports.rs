use crate::walk::rewrite_operands;
use sa_ir::{Diagnostic, InstructionKind, Operand, Program, Trap};
use std::collections::{HashMap, HashSet};

fn push_duplicate(diags: &mut Vec<Diagnostic>, diagnostic: Diagnostic) {
    if !diags.contains(&diagnostic) {
        diags.push(diagnostic);
    }
}

pub(crate) fn merge(dst: &mut Program, src: Program, path: &str, diags: &mut Vec<Diagnostic>) {
    for constant in src.consts {
        if !dst.consts.iter().any(|existing| existing.name == constant.name) {
            dst.consts.push(constant);
        }
    }
    for definition in src.defs {
        match dst.defs.iter().find(|existing| existing.name == definition.name) {
            Some(existing) if existing.value != definition.value => {
                push_duplicate(
                    diags,
                    Diagnostic::new(
                        Trap::DuplicateDef,
                        "duplicate definition detected during flattening",
                        definition.loc.clone(),
                    )
                    .with_hint(format!("conflicting definition '{}'", definition.name)),
                );
            }
            Some(_) => {}
            None => dst.defs.push(definition),
        }
    }
    for extern_sig in src.externs {
        if !dst.externs.iter().any(|existing| existing.name == extern_sig.name) {
            dst.externs.push(extern_sig);
        }
    }
    dst.locs.extend(src.locs);
    for bind in src.npm_binds {
        if !dst.npm_binds.iter().any(|existing| existing.extern_name == bind.extern_name) {
            dst.npm_binds.push(bind);
        }
    }

    let mut names: HashSet<String> = dst
        .functions
        .iter()
        .map(|function| function.sig.name.clone())
        .collect();
    let mut labels: HashSet<String> = dst
        .functions
        .iter()
        .flat_map(|function| function.labels().into_iter().map(str::to_string))
        .collect();
    let mut prefixes: HashSet<String> = HashSet::new();

    for mut function in src.functions {
        if names.contains(&function.sig.name) {
            continue;
        }
        let collides = function
            .labels()
            .iter()
            .any(|label| labels.contains(*label));
        if collides {
            let prefix = unique_prefix(path, &prefixes);
            prefixes.insert(prefix.clone());
            let mut renamed = HashMap::new();
            for block in &mut function.blocks {
                let old = block.label.clone();
                let new = format!("{}{}", prefix, old);
                renamed.insert(old, new.clone());
                block.label = new;
            }
            for block in &mut function.blocks {
                for instruction in &mut block.instrs {
                    rename_targets(&mut instruction.kind, &renamed);
                    rewrite_operands(&mut instruction.kind, &mut |operand| {
                        if let Operand::Label(name) = operand {
                            if let Some(new) = renamed.get(name) {
                                return Operand::Label(new.clone());
                            }
                        }
                        operand.clone()
                    });
                }
            }
        }
        for label in function.labels() {
            labels.insert(label.to_string());
        }
        names.insert(function.sig.name.clone());
        dst.functions.push(function);
    }
}

fn rename_targets(kind: &mut InstructionKind, renamed: &HashMap<String, String>) {
    match kind {
        InstructionKind::Jmp { target } => {
            if let Some(new) = renamed.get(target) {
                *target = new.clone();
            }
        }
        InstructionKind::Br {
            then_label,
            else_label,
            ..
        } => {
            if let Some(new) = renamed.get(then_label) {
                *then_label = new.clone();
            }
            if let Some(new) = renamed.get(else_label) {
                *else_label = new.clone();
            }
        }
        InstructionKind::BrNull { target, else_target, .. } => {
            if let Some(new) = renamed.get(target) {
                *target = new.clone();
            }
            if let Some(other) = else_target {
                if let Some(new) = renamed.get(other) {
                    *other = new.clone();
                }
            }
        }
        _ => {}
    }
}

fn module_base(path: &str) -> String {
    let stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(path);
    let mut out: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out.push_str("mod");
    }
    out.push('_');
    out
}

fn unique_prefix(path: &str, used: &HashSet<String>) -> String {
    let base = module_base(path);
    if !used.contains(&base) {
        return base;
    }
    let mut counter = 2;
    loop {
        let candidate = format!("{}_{}", base, counter);
        if !used.contains(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{
        Block, ConstDecl, ConstValue, Function, Instruction, Signature, SourceLoc,
    };

    fn function(name: &str, label: &str, kind: InstructionKind) -> Function {
        let loc = SourceLoc::new("x.sa", 1, 1);
        let mut block = Block::new(label, loc.clone());
        block.instrs.push(Instruction::new(kind, loc.clone(), "test"));
        let mut function = Function::new(Signature::new(name), loc);
        function.blocks.push(block);
        function
    }

    #[test]
    fn skips_duplicate_names() {
        let mut dst = Program::new();
        dst.consts.push(ConstDecl::new("A", ConstValue::Int(1)));
        dst.functions
            .push(function("foo", "entry", InstructionKind::Nop));
        dst.externs.push(Signature::new("ext"));

        let mut src = Program::new();
        src.consts.push(ConstDecl::new("A", ConstValue::Int(2)));
        src.consts.push(ConstDecl::new("B", ConstValue::Int(3)));
        src.externs.push(Signature::new("ext"));
        src.externs.push(Signature::new("other"));
        src.functions
            .push(function("foo", "entry", InstructionKind::Nop));
        src.functions
            .push(function("bar", "other", InstructionKind::Nop));

        merge(&mut dst, src, "lib.sa", &mut Vec::new());

        let const_names: Vec<&str> = dst.consts.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(const_names, vec!["A", "B"]);
        let func_names: Vec<&str> = dst.functions.iter().map(|f| f.name()).collect();
        assert_eq!(func_names, vec!["foo", "bar"]);
        assert_eq!(dst.externs.len(), 2);
    }

    #[test]
    fn prefixes_colliding_labels_and_rewrites_targets() {
        let mut dst = Program::new();
        dst.functions.push(function(
            "foo",
            "entry",
            InstructionKind::Jmp {
                target: "entry".to_string(),
            },
        ));

        let mut src = Program::new();
        src.functions.push(function(
            "bar",
            "entry",
            InstructionKind::Br {
                cond: Operand::Int(1),
                then_label: "entry".to_string(),
                else_label: "exit".to_string(),
            },
        ));
        merge(&mut dst, src, "sub/bar.sa", &mut Vec::new());

        let imported = &dst.functions[1];
        assert_eq!(imported.blocks[0].label, "bar_entry");
        match &imported.blocks[0].instrs[0].kind {
            InstructionKind::Br {
                then_label,
                else_label,
                ..
            } => {
                assert_eq!(then_label, "bar_entry");
                assert_eq!(else_label, "exit");
            }
            _ => panic!("wrong kind"),
        }
        match &dst.functions[0].blocks[0].instrs[0].kind {
            InstructionKind::Jmp { target } => assert_eq!(target, "entry"),
            _ => panic!("wrong kind"),
        }
    }

    #[test]
    fn keeps_labels_when_no_collision() {
        let mut dst = Program::new();
        dst.functions
            .push(function("foo", "start", InstructionKind::Nop));
        let mut src = Program::new();
        src.functions
            .push(function("bar", "entry", InstructionKind::Nop));
        merge(&mut dst, src, "bar.sa", &mut Vec::new());
        assert_eq!(dst.functions[1].blocks[0].label, "entry");
    }

    #[test]
    fn generates_unique_prefixes() {
        let mut used = HashSet::new();
        let first = unique_prefix("math.sa", &used);
        used.insert(first.clone());
        let second = unique_prefix("dir/math.sa", &used);
        assert_eq!(first, "math_");
        assert_eq!(second, "math__2");
    }
}
