pub mod cfg;
pub mod rules;
pub mod state;

#[cfg(test)]
mod tests;

use sa_ir::{Diagnostic, Function, Program, Trap};
use std::collections::HashMap;
use std::collections::VecDeque;

use state::{FnState, RegKind, RegState};

#[derive(Debug, Clone)]
pub struct VerifyOptions {
    pub max_gas: u64,
    pub check_leaks: bool,
    pub check_capabilities: bool,
    pub check_phi: bool,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        Self {
            max_gas: 100_000_000,
            check_leaks: true,
            check_capabilities: true,
            check_phi: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VerifyReport {
    pub gas_used: u64,
    pub functions_checked: usize,
    pub instructions_checked: usize,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Default)]
pub struct DiagSink {
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
}

impl DiagSink {
    fn key(diagnostic: &Diagnostic) -> (Trap, String, u32) {
        (diagnostic.trap, diagnostic.message.clone(), diagnostic.loc.line)
    }

    pub fn push_error(&mut self, diagnostic: Diagnostic) {
        let key = Self::key(&diagnostic);
        if !self.errors.iter().any(|existing| Self::key(existing) == key) {
            self.errors.push(diagnostic);
        }
    }

    pub fn push_warning(&mut self, diagnostic: Diagnostic) {
        let key = Self::key(&diagnostic);
        if !self.warnings.iter().any(|existing| Self::key(existing) == key) {
            self.warnings.push(diagnostic);
        }
    }
}

fn gas_total<'a>(functions: impl Iterator<Item = &'a Function>) -> u64 {
    functions
        .flat_map(|function| function.all_instructions())
        .map(|instruction| instruction.gas_cost())
        .sum()
}

fn check_gas(used: u64, function: &Function, opts: &VerifyOptions, sink: &mut DiagSink) {
    if used <= opts.max_gas {
        return;
    }
    let diagnostic = Diagnostic::new(
        Trap::GasExceeded,
        format!("gas budget exceeded: used {used} > limit {}", opts.max_gas),
        function.loc.clone(),
    )
    .in_function(function.name())
    .with_hint("raise VerifyOptions::max_gas or reduce the instruction count");
    sink.push_error(diagnostic);
}

fn initial_state(function: &Function) -> FnState {
    let mut state = FnState::default();
    for param in &function.sig.params {
        state.set(&param.name, RegState::Owned { kind: RegKind::Param });
    }
    for local in &function.locals {
        if state.state_of(local) == RegState::Undefined {
            state.set(local, RegState::Owned { kind: RegKind::Value });
        }
    }
    state
}

fn phi_warning(
    function: &Function,
    block_index: usize,
    conflicts: Vec<(String, RegState, RegState)>,
    sink: &mut DiagSink,
    opts: &VerifyOptions,
) {
    if !opts.check_phi {
        return;
    }
    let block = &function.blocks[block_index];
    for (name, left, right) in conflicts {
        let diagnostic = Diagnostic::new(
            Trap::PhiStateConflict,
            format!(
                "phi merge conflict for register '{name}': {:?} versus {:?}",
                left, right
            ),
            block.loc.clone(),
        )
        .in_function(function.name())
        .for_register(name)
        .with_hint("make the register state agree on all incoming control-flow paths");
        sink.push_warning(diagnostic);
    }
}

fn check_function(function: &Function, program: &Program, opts: &VerifyOptions, sink: &mut DiagSink) {
    let cfg = cfg::build(function, sink);
    if cfg.successors.is_empty() {
        return;
    }

    let mut in_states: Vec<Option<FnState>> = vec![None; cfg.successors.len()];
    in_states[0] = Some(initial_state(function));
    let mut worklist = VecDeque::new();
    worklist.push_back(0usize);
    let mut atomics: HashMap<String, sa_ir::AtomicOrdering> = HashMap::new();
    let mut steps = 0usize;

    while let Some(index) = worklist.pop_front() {
        steps += 1;
        if steps > 100_000 {
            break;
        }
        let mut state = match &in_states[index] {
            Some(existing) => existing.clone(),
            None => continue,
        };
        for instruction in &function.blocks[index].instrs {
            let mut context = rules::RuleCtx {
                program,
                opts,
                sink,
                func_name: function.name(),
                signature: &function.sig,
                atomics: &mut atomics,
            };
            rules::transfer(instruction, &mut state, &mut context);
        }
        for &successor in &cfg.successors[index] {
            let merged = match &in_states[successor] {
                None => state.clone(),
                Some(previous) => {
                    let (merged, conflicts) = previous.merge(&state);
                    phi_warning(function, successor, conflicts, sink, opts);
                    merged
                }
            };
            let changed = in_states[successor]
                .as_ref()
                .map(|previous| !previous.is_same(&merged))
                .unwrap_or(true);
            if changed {
                in_states[successor] = Some(merged);
                if !worklist.contains(&successor) {
                    worklist.push_back(successor);
                }
            }
        }
    }
}

pub fn verify(program: &Program, opts: &VerifyOptions) -> Result<VerifyReport, Vec<Diagnostic>> {
    let mut sink = DiagSink::default();
    let gas = gas_total(program.functions.iter());
    if let Some(first) = program.functions.first() {
        check_gas(gas, first, opts, &mut sink);
    }
    let mut report = VerifyReport { gas_used: gas, ..VerifyReport::default() };
    for bind in &program.npm_binds {
        if !program.has_extern(&bind.extern_name) {
            sink.push_error(
                Diagnostic::new(
                    Trap::UnknownRegister,
                    format!("#npm_bind target '@{}' is not declared as @extern", bind.extern_name),
                    bind.loc.clone(),
                )
                .with_hint("declare the extern before binding it with #npm_bind"),
            );
        }
    }
    for function in &program.functions {
        check_function(function, program, opts, &mut sink);
        report.functions_checked += 1;
        report.instructions_checked += function.all_instructions().len();
    }
    if !sink.errors.is_empty() {
        return Err(sink.errors);
    }
    report.warnings = sink.warnings;
    Ok(report)
}

pub fn verify_function(
    function: &Function,
    program: &Program,
    opts: &VerifyOptions,
) -> Result<VerifyReport, Vec<Diagnostic>> {
    let mut sink = DiagSink::default();
    let gas = gas_total(std::iter::once(function));
    check_gas(gas, function, opts, &mut sink);
    check_function(function, program, opts, &mut sink);
    if !sink.errors.is_empty() {
        return Err(sink.errors);
    }
    Ok(VerifyReport {
        gas_used: gas,
        functions_checked: 1,
        instructions_checked: function.all_instructions().len(),
        warnings: sink.warnings,
    })
}
