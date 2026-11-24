use crate::DiagSink;
use sa_ir::{Diagnostic, Function, Instruction, InstructionKind, SourceLoc, Trap};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Cfg {
    pub labels: HashMap<String, usize>,
    pub successors: Vec<Vec<usize>>,
    pub fallthrough: Vec<bool>,
}

impl Cfg {
    pub fn target_label(ins: &Instruction) -> Option<Vec<&str>> {
        match &ins.kind {
            InstructionKind::Jmp { target } => Some(vec![target.as_str()]),
            InstructionKind::Br { then_label, else_label, .. } => {
                Some(vec![then_label.as_str(), else_label.as_str()])
            }
            InstructionKind::BrNull { target, .. } => Some(vec![target.as_str()]),
            _ => None,
        }
    }

    pub fn is_terminator(ins: &Instruction) -> bool {
        matches!(
            ins.kind,
            InstructionKind::Jmp { .. }
                | InstructionKind::Br { .. }
                | InstructionKind::BrNull { .. }
                | InstructionKind::Return { .. }
                | InstructionKind::EarlyReturn { .. }
                | InstructionKind::Panic { .. }
        )
    }
}

pub fn build(f: &Function, sink: &mut DiagSink) -> Cfg {
    let mut labels: HashMap<String, usize> = HashMap::new();
    for (index, block) in f.blocks.iter().enumerate() {
        if labels.insert(block.label.clone(), index).is_some() {
            let diagnostic = Diagnostic::new(
                Trap::DuplicateLabel,
                format!("duplicate label '{}'", block.label),
                block.loc.clone(),
            )
            .in_function(f.name())
            .with_hint("each basic block label must be unique within a function");
            sink.push_error(diagnostic);
        }
    }

    let mut successors = vec![Vec::new(); f.blocks.len()];
    let mut fallthrough = vec![false; f.blocks.len()];

    for (index, block) in f.blocks.iter().enumerate() {
        let terminator = block.instrs.iter().find(|ins| Cfg::is_terminator(ins));
        match terminator {
            Some(ins) => {
                for label in Cfg::target_label(ins).unwrap_or_default() {
                    match labels.get(label) {
                        Some(&target) => {
                            if !successors[index].contains(&target) {
                                successors[index].push(target);
                            }
                        }
                        None => {
                            let diagnostic = Diagnostic::new(
                                Trap::UnknownRegister,
                                format!("jump to unknown label '{label}'"),
                                ins.loc.clone(),
                            )
                            .in_function(f.name())
                            .with_hint(format!("declare '{label}' as a basic block label"));
                            sink.push_error(diagnostic);
                        }
                    }
                }
            }
            None => {
                if index + 1 < f.blocks.len() {
                    fallthrough[index] = true;
                    successors[index].push(index + 1);
                    let loc: SourceLoc = block
                        .instrs
                        .last()
                        .map(|ins| ins.loc.clone())
                        .unwrap_or_else(|| block.loc.clone());
                    let diagnostic = Diagnostic::new(
                        Trap::FallthroughForbidden,
                        format!("block '{}' has no terminator", block.label),
                        loc,
                    )
                    .in_function(f.name())
                    .with_hint("end the block with jmp, br, br_null, return or panic");
                    sink.push_warning(diagnostic);
                }
            }
        }
    }

    Cfg { labels, successors, fallthrough }
}
