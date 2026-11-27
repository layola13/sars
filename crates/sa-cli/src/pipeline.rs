use sa_flatten::{flatten_source, FlattenOptions, FsResolver};
use sa_ir::{Diagnostic, Program, SourceLoc, Trap};
use sa_verify::{verify, VerifyOptions};
use std::path::{Path, PathBuf};

pub struct Compilation {
    pub program: Program,
    pub report: Option<sa_verify::VerifyReport>,
}

trait IntoDiagnostics {
    fn into_diagnostics(self) -> Vec<Diagnostic>;
}

impl IntoDiagnostics for Vec<Diagnostic> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self
    }
}

impl IntoDiagnostics for Diagnostic {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        vec![self]
    }
}

trait IntoProgram {
    fn into_program(self, file: &str) -> Result<Program, Vec<Diagnostic>>;
}

impl IntoProgram for Program {
    fn into_program(self, _file: &str) -> Result<Program, Vec<Diagnostic>> {
        Ok(self)
    }
}

impl IntoProgram for String {
    fn into_program(self, file: &str) -> Result<Program, Vec<Diagnostic>> {
        sa_parser::parse_source(&self, file)
    }
}

impl IntoProgram for &str {
    fn into_program(self, file: &str) -> Result<Program, Vec<Diagnostic>> {
        sa_parser::parse_source(self, file)
    }
}

fn push_root(roots: &mut Vec<PathBuf>, path: PathBuf) {
    if !roots.contains(&path) {
        roots.push(path);
    }
}

fn add_std_roots(roots: &mut Vec<PathBuf>, std_dir: &Path) {
    if !std_dir.is_dir() {
        return;
    }
    push_root(roots, std_dir.to_path_buf());
    if let Ok(entries) = std::fs::read_dir(std_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                push_root(roots, path);
            }
        }
    }
}

pub fn resolve_roots(path: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let target = Path::new(path);
    let absolute = std::fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
    let mut dir = absolute.parent().map(Path::to_path_buf);
    for _ in 0..5 {
        let current = match dir {
            Some(d) => d,
            None => break,
        };
        push_root(&mut roots, current.clone());
        add_std_roots(&mut roots, &current.join("sa_std"));
        add_std_roots(&mut roots, &current.join("sla_std"));
        dir = current.parent().map(Path::to_path_buf);
    }
    if let Ok(cwd) = std::env::current_dir() {
        push_root(&mut roots, cwd.clone());
        add_std_roots(&mut roots, &cwd.join("sa_std"));
        add_std_roots(&mut roots, &cwd.join("sla_std"));
    }
    if let Ok(value) = std::env::var("SA_STD") {
        for part in value.split(|c| c == ':' || c == ';') {
            if !part.is_empty() {
                push_root(&mut roots, PathBuf::from(part));
            }
        }
    }
    roots
}

pub fn compile(path: &str, do_verify: bool) -> Result<Compilation, Vec<Diagnostic>> {
    compile_with_options(path, do_verify, VerifyOptions::default())
}

pub fn compile_with_options(
    path: &str,
    do_verify: bool,
    opts: VerifyOptions,
) -> Result<Compilation, Vec<Diagnostic>> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        vec![Diagnostic::new(
            Trap::ImportResolutionFailed,
            format!("cannot read {path}: {e}"),
            SourceLoc::new(path, 0, 0),
        )]
    })?;
    let resolver = FsResolver::new(resolve_roots(path));
    let flat = flatten_source(&source, path, &resolver, &FlattenOptions::default())
        .map_err(|e| e.into_diagnostics())?;
    let program = flat.into_program(path)?;
    let report = if do_verify {
        match verify(&program, &opts) {
            Ok(report) => Some(report),
            Err(diags) => return Err(diags.into_diagnostics()),
        }
    } else {
        None
    };
    Ok(Compilation { program, report })
}

pub fn gas_estimate(program: &Program) -> u64 {
    program
        .functions
        .iter()
        .flat_map(|f| f.all_instructions())
        .map(|i| i.gas_cost())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{Block, Instruction, InstructionKind, Signature, SourceLoc};

    #[test]
    fn diagnostic_conversions() {
        let d = Diagnostic::new(Trap::ParseError, "bad", SourceLoc::unknown());
        let v: Vec<Diagnostic> = d.into_diagnostics();
        assert_eq!(v.len(), 1);
        let v: Vec<Diagnostic> = vec![v[0].clone(), v[0].clone()].into_diagnostics();
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn program_passes_through() {
        let sig = Signature::new("main");
        let mut f = sa_ir::Function::new(sig, SourceLoc::unknown());
        f.blocks.push(Block::new("entry", SourceLoc::unknown()));
        let mut p = Program::new();
        p.functions.push(f);
        let program: Program = p.into_program("x.sa").unwrap();
        assert_eq!(program.functions[0].sig.name, "main");
    }

    #[test]
    fn gas_estimate_sums_instructions() {
        let mut p = Program::new();
        let sig = Signature::new("main");
        let mut f = sa_ir::Function::new(sig, SourceLoc::unknown());
        let mut block = Block::new("entry", SourceLoc::unknown());
        block.instrs.push(Instruction::new(
            InstructionKind::Nop,
            SourceLoc::unknown(),
            "nop",
        ));
        block.instrs.push(Instruction::new(
            InstructionKind::Return { value: None },
            SourceLoc::unknown(),
            "return",
        ));
        f.blocks.push(block);
        p.functions.push(f);
        assert_eq!(gas_estimate(&p), 2);
    }

    #[test]
    fn roots_are_deduplicated() {
        let mut roots = Vec::new();
        let p = PathBuf::from("a");
        push_root(&mut roots, p.clone());
        push_root(&mut roots, p);
        assert_eq!(roots.len(), 1);
    }
}
