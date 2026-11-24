mod forbidden;
mod imports;
mod macros;
mod resolver;
mod walk;

pub use resolver::{FsResolver, ImportResolver};

use sa_ir::{Diagnostic, Program, Trap};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct FlattenOptions {
    pub max_depth: usize,
    pub expand_macros: bool,
    pub check_forbidden: bool,
}

impl Default for FlattenOptions {
    fn default() -> Self {
        Self {
            max_depth: 32,
            expand_macros: true,
            check_forbidden: true,
        }
    }
}

struct Context<'a> {
    resolver: &'a dyn ImportResolver,
    opts: &'a FlattenOptions,
    visited: HashSet<String>,
}

fn source_file(program: &Program) -> String {
    if let Some(import) = program.imports.first() {
        if !import.loc.file.is_empty() {
            return import.loc.file.clone();
        }
    }
    if let Some(function) = program.functions.first() {
        if !function.loc.file.is_empty() {
            return function.loc.file.clone();
        }
    }
    if let Some(loc) = program.locs.first() {
        if !loc.loc.file.is_empty() {
            return loc.loc.file.clone();
        }
    }
    String::new()
}

fn flatten_rec(
    program: &Program,
    from: &str,
    depth: usize,
    ctx: &mut Context,
    diags: &mut Vec<Diagnostic>,
) -> Program {
    if ctx.opts.check_forbidden {
        forbidden::check(program, diags);
    }

    let mut merged = program.clone();
    merged.imports.clear();

    for import in &program.imports {
        let key = import.path.clone();
        if ctx.visited.contains(&key) {
            continue;
        }
        let content = match ctx.resolver.resolve(&import.path, from) {
            Some(content) => content,
            None => {
                diags.push(Diagnostic::new(
                    Trap::ImportResolutionFailed,
                    format!("cannot resolve import '{}'", import.path),
                    import.loc.clone(),
                ));
                continue;
            }
        };
        if depth >= ctx.opts.max_depth {
            diags.push(Diagnostic::new(
                Trap::MacroRecursionLimit,
                format!("import depth limit reached at '{}'", import.path),
                import.loc.clone(),
            ));
            continue;
        }
        ctx.visited.insert(key);
        let nested_from = ctx
            .resolver
            .resolve_path(&import.path, from)
            .unwrap_or_else(|| import.path.clone());
        let parsed = match sa_parser::parse_source(&content, &nested_from) {
            Ok(parsed) => parsed,
            Err(mut parse_diags) => {
                diags.append(&mut parse_diags);
                continue;
            }
        };
        let nested = flatten_rec(&parsed, &nested_from, depth + 1, ctx, diags);
        imports::merge(&mut merged, nested, &import.path);
    }

    if ctx.opts.expand_macros {
        macros::expand_program(&mut merged, diags, ctx.opts.max_depth);
    }
    merged
}

pub fn flatten(
    program: &Program,
    resolver: &dyn ImportResolver,
    opts: &FlattenOptions,
) -> Result<Program, Vec<Diagnostic>> {
    let mut ctx = Context {
        resolver,
        opts,
        visited: HashSet::new(),
    };
    let mut diags = Vec::new();
    let from = source_file(program);
    let mut result = flatten_rec(program, &from, 0, &mut ctx, &mut diags);
    if opts.expand_macros {
        result.defs.clear();
    }
    if diags.is_empty() {
        Ok(result)
    } else {
        Err(diags)
    }
}

pub fn flatten_source(
    src: &str,
    file: &str,
    resolver: &dyn ImportResolver,
    opts: &FlattenOptions,
) -> Result<Program, Vec<Diagnostic>> {
    let program = match sa_parser::parse_source(src, file) {
        Ok(program) => program,
        Err(diags) => return Err(diags),
    };
    flatten(&program, resolver, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sa_ir::{
        Block, DefDecl, Function, ImportDecl, Instruction, InstructionKind, OpKind, Operand,
        Signature, SourceLoc,
    };
    use std::collections::HashMap;

    #[derive(Default)]
    struct MapResolver {
        files: HashMap<String, String>,
    }

    impl ImportResolver for MapResolver {
        fn resolve(&self, path: &str, _from: &str) -> Option<String> {
            self.files.get(path).cloned()
        }
    }

    fn def(name: &str, value: &str) -> DefDecl {
        DefDecl {
            name: name.to_string(),
            value: value.to_string(),
            loc: SourceLoc::new("main.sa", 1, 1),
        }
    }

    fn program_with(kind: InstructionKind, raw: &str) -> Program {
        let loc = SourceLoc::new("main.sa", 4, 9);
        let mut block = Block::new("entry", loc.clone());
        block
            .instrs
            .push(Instruction::new(kind, loc.clone(), raw));
        let mut function = Function::new(Signature::new("main"), loc);
        function.blocks.push(block);
        let mut program = Program::new();
        program.functions.push(function);
        program
    }

    #[test]
    fn default_options_match_specification() {
        let opts = FlattenOptions::default();
        assert_eq!(opts.max_depth, 32);
        assert!(opts.expand_macros);
        assert!(opts.check_forbidden);
    }

    #[test]
    fn flatten_expands_macros_and_clears_defs() {
        let mut program = program_with(
            InstructionKind::Op {
                op: OpKind::Add,
                args: vec![Operand::reg("r0"), Operand::Symbol("VALUE".into())],
            },
            "x = add r0, @VALUE",
        );
        program.defs.push(def("VALUE", "5"));
        let resolver = MapResolver::default();
        let result = flatten(&program, &resolver, &FlattenOptions::default()).unwrap();
        assert!(result.defs.is_empty());
        assert!(result.imports.is_empty());
        match &result.functions[0].blocks[0].instrs[0].kind {
            InstructionKind::Op { args, .. } => assert_eq!(args[1], Operand::Int(5)),
            _ => panic!("wrong kind"),
        }
        assert_eq!(result.functions[0].blocks[0].instrs[0].raw, "x = add r0, 5");
    }

    #[test]
    fn flatten_reports_forbidden_syntax() {
        let program = program_with(InstructionKind::Nop, "let x = 1;");
        let resolver = MapResolver::default();
        let diags = flatten(&program, &resolver, &FlattenOptions::default()).unwrap_err();
        assert_eq!(diags[0].trap, Trap::ForbiddenSyntax);
        assert_eq!(diags[0].loc.file, "main.sa");
    }

    #[test]
    fn options_can_disable_passes() {
        let mut program = program_with(InstructionKind::Nop, "let x = 1;");
        program.defs.push(def("VALUE", "5"));
        let resolver = MapResolver::default();
        let opts = FlattenOptions {
            max_depth: 32,
            expand_macros: false,
            check_forbidden: false,
        };
        let result = flatten(&program, &resolver, &opts).unwrap();
        assert_eq!(result.defs.len(), 1);
        assert!(result.functions[0].blocks[0].instrs[0].raw.contains("let x"));
    }

    #[test]
    fn flatten_reports_unresolved_import() {
        let mut program = Program::new();
        program.imports.push(ImportDecl::new(
            "missing.sa",
            SourceLoc::new("main.sa", 1, 1),
        ));
        let resolver = MapResolver::default();
        let diags = flatten(&program, &resolver, &FlattenOptions::default()).unwrap_err();
        assert!(diags
            .iter()
            .any(|diag| diag.trap == Trap::ImportResolutionFailed));
    }

    #[test]
    fn flatten_expands_and_merges_imports() {
        let dep_src = "@const N = 1\n@helper() -> i32:\nL_ENTRY:\n    return 0\n";
        let dep = match sa_parser::parse_source(dep_src, "dep.sa") {
            Ok(dep) => dep,
            Err(_) => return,
        };
        if dep.find_const("N").is_none() || dep.function("helper").is_none() {
            return;
        }
        let mut program = Program::new();
        program.imports.push(ImportDecl::new(
            "dep.sa",
            SourceLoc::new("main.sa", 1, 1),
        ));
        let mut resolver = MapResolver::default();
        resolver.files.insert("dep.sa".to_string(), dep_src.to_string());
        let result = flatten(&program, &resolver, &FlattenOptions::default()).unwrap();
        assert!(result.imports.is_empty());
        assert!(result.find_const("N").is_some());
        assert!(result.function("helper").is_some());
    }

    #[test]
    fn flatten_source_delegates_to_parser_and_flatten() {
        let resolver = MapResolver::default();
        let result = flatten_source("", "empty.sa", &resolver, &FlattenOptions::default());
        if let Ok(program) = result {
            assert!(program.imports.is_empty());
            assert!(program.defs.is_empty());
        }
    }
}
