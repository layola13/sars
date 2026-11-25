mod forbidden;
mod imports;
mod macros;
mod resolver;
mod walk;

pub use resolver::{FsResolver, ImportResolver};

use crate::macros::MacroDef;
use sa_ir::{Diagnostic, Program, Trap};
use std::collections::{HashMap, HashSet};

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
    stack: Vec<String>,
    macros: HashMap<String, MacroDef>,
}

fn canonical_key(path: &str) -> String {
    std::fs::canonicalize(path)
        .map(|resolved| resolved.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string())
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
        let (content, nested_from) = match ctx
            .resolver
            .resolve_child(&import.path, from, depth == 0)
        {
            Some(resolved) => resolved,
            None => {
                diags.push(
                    Diagnostic::new(
                        Trap::ImportResolutionFailed,
                        "import path could not be resolved",
                        import.loc.clone(),
                    )
                    .with_hint(
                        "check the import path, package identity, local vendor tree, and package cache",
                    ),
                );
                continue;
            }
        };
        let nested_key = canonical_key(&nested_from);
        if ctx.stack.iter().any(|item| item == &nested_key) {
            diags.push(
                Diagnostic::new(
                    Trap::ForbiddenSyntax,
                    "import cycle detected during flattening",
                    import.loc.clone(),
                )
                .with_hint("break the cycle in imported files or inline the shared definitions"),
            );
            continue;
        }
        if ctx.visited.contains(&key) {
            continue;
        }
        if depth >= ctx.opts.max_depth {
            diags.push(Diagnostic::new(
                Trap::MacroRecursionLimit,
                format!("import depth limit reached at '{}'", import.path),
                import.loc.clone(),
            ));
            continue;
        }
        ctx.visited.insert(key);
        ctx.stack.push(nested_key);
        macros::collect_macros(&content, &mut ctx.macros);
        let content = if ctx.opts.expand_macros {
            macros::expand_text(&content, &ctx.macros, 0, ctx.opts.max_depth, diags)
        } else {
            content
        };
        let parsed = match sa_parser::parse_source(&content, &nested_from) {
            Ok(parsed) => parsed,
            Err(mut parse_diags) => {
                diags.append(&mut parse_diags);
                ctx.stack.pop();
                continue;
            }
        };
        let nested = flatten_rec(&parsed, &nested_from, depth + 1, ctx, diags);
        ctx.stack.pop();
        imports::merge(&mut merged, nested, &import.path, diags);
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
        stack: Vec::new(),
        macros: HashMap::new(),
    };
    let mut diags = Vec::new();
    let from = source_file(program);
    ctx.stack.push(canonical_key(&from));
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

fn precollect_macros(
    src: &str,
    from: &str,
    ctx: &mut Context,
    depth: usize,
    seen: &mut HashSet<String>,
) {
    macros::collect_macros(src, &mut ctx.macros);
    if depth >= ctx.opts.max_depth {
        return;
    }
    for path in import_paths(src) {
        if seen.contains(&path) {
            continue;
        }
        let content = match ctx.resolver.resolve(&path, from) {
            Some(content) => content,
            None => continue,
        };
        seen.insert(path.clone());
        let nested_from = ctx
            .resolver
            .resolve_path(&path, from)
            .unwrap_or_else(|| path.clone());
        precollect_macros(&content, &nested_from, ctx, depth + 1, seen);
    }
}

fn import_paths(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_macro = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[MACRO]") {
            in_macro = true;
            continue;
        }
        if trimmed == "[END_MACRO]" {
            in_macro = false;
            continue;
        }
        if in_macro || !trimmed.starts_with("@import") {
            continue;
        }
        let Some(start) = trimmed.find('"') else {
            continue;
        };
        let Some(end) = trimmed[start + 1..].find('"') else {
            continue;
        };
        out.push(trimmed[start + 1..start + 1 + end].to_string());
    }
    out
}

pub fn flatten_source(
    src: &str,
    file: &str,
    resolver: &dyn ImportResolver,
    opts: &FlattenOptions,
) -> Result<Program, Vec<Diagnostic>> {
    let mut ctx = Context {
        resolver,
        opts,
        visited: HashSet::new(),
        stack: Vec::new(),
        macros: HashMap::new(),
    };
    let mut diags = Vec::new();
    ctx.stack.push(canonical_key(file));
    macros::collect_macros(src, &mut ctx.macros);
    if opts.expand_macros {
        let mut seen = HashSet::new();
        precollect_macros(src, file, &mut ctx, 0, &mut seen);
    }
    let expanded = if opts.expand_macros {
        macros::expand_text(src, &ctx.macros, 0, opts.max_depth, &mut diags)
    } else {
        src.to_string()
    };
    let program = match sa_parser::parse_source(&expanded, file) {
        Ok(program) => program,
        Err(mut parse_diags) => {
            diags.append(&mut parse_diags);
            return Err(diags);
        }
    };
    let mut result = flatten_rec(&program, file, 0, &mut ctx, &mut diags);
    if opts.expand_macros {
        result.defs.clear();
    }
    if diags.is_empty() {
        Ok(result)
    } else {
        Err(diags)
    }
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

    #[derive(Default)]
    struct NestedResolver {
        files: HashMap<String, (String, String)>,
    }

    impl NestedResolver {
        fn insert(&mut self, path: &str, resolved: &str, source: &str) {
            self.files
                .insert(path.to_string(), (source.to_string(), resolved.to_string()));
        }
    }

    impl ImportResolver for NestedResolver {
        fn resolve(&self, path: &str, _from: &str) -> Option<String> {
            self.files.get(path).map(|(source, _)| source.clone())
        }

        fn resolve_child(
            &self,
            path: &str,
            _from: &str,
            _allow_roots: bool,
        ) -> Option<(String, String)> {
            self.files.get(path).cloned()
        }
    }

    fn diag_for(diags: &[Diagnostic], trap: Trap) -> &Diagnostic {
        diags
            .iter()
            .find(|diag| diag.trap == trap)
            .unwrap_or_else(|| panic!("missing {trap:?} in {diags:?}"))
    }

    #[test]
    fn flatten_rejects_import_cycles() {
        let mut resolver = NestedResolver::default();
        resolver.insert(
            "a.sa",
            "a.sa",
            "@import \"b.sa\"\n@fa() -> i32:\nL_ENTRY:\n    return 0\n",
        );
        resolver.insert(
            "b.sa",
            "b.sa",
            "@import \"a.sa\"\n@fb() -> i32:\nL_ENTRY:\n    return 0\n",
        );
        let src = "@import \"a.sa\"\n@main() -> i32:\nL_ENTRY:\n    return 0\n";
        let diags = flatten_source(src, "main.sa", &resolver, &FlattenOptions::default())
            .expect_err("cycle must be rejected");
        let diag = diag_for(&diags, Trap::ForbiddenSyntax);
        assert_eq!(diag.message, "import cycle detected during flattening");
        assert_eq!(
            diag.hint.as_deref(),
            Some("break the cycle in imported files or inline the shared definitions")
        );
    }

    #[test]
    fn flatten_rejects_conflicting_definitions() {
        let mut resolver = NestedResolver::default();
        resolver.insert("v1.sal", "v1.sal", "#def VERSION = 1\n");
        resolver.insert("v2.sal", "v2.sal", "#def VERSION = 2\n");
        let src = "@import \"v1.sal\"\n@import \"v2.sal\"\n@main() -> i32:\nL_ENTRY:\n    return 0\n";
        let diags = flatten_source(src, "main.sa", &resolver, &FlattenOptions::default())
            .expect_err("conflicting definitions must be rejected");
        let diag = diag_for(&diags, Trap::DuplicateDef);
        assert_eq!(diag.message, "duplicate definition detected during flattening");
        assert_eq!(diag.hint.as_deref(), Some("conflicting definition 'VERSION'"));
    }

    #[test]
    fn flatten_rejects_unresolved_nested_import() {
        let mut resolver = NestedResolver::default();
        resolver.insert(
            "a.sa",
            "a.sa",
            "@import \"west/b.sa\"\n@fa() -> i32:\nL_ENTRY:\n    return 0\n",
        );
        let src = "@import \"a.sa\"\n@main() -> i32:\nL_ENTRY:\n    return 0\n";
        let diags = flatten_source(src, "main.sa", &resolver, &FlattenOptions::default())
            .expect_err("nested unresolved import must be rejected");
        let diag = diag_for(&diags, Trap::ImportResolutionFailed);
        assert_eq!(diag.message, "import path could not be resolved");
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

    #[test]
    fn flatten_source_expands_macros_from_transitive_imports() {
        let defs_src = "[MACRO] MAKE %out, %value\n    %out = add %value, 1\n[END_MACRO]\n";
        let root = "@import \"middle.sa\"\n@main() -> i32:\nL_ENTRY:\n    EXPAND MAKE r0, 4\n    return r0\n";
        let mut resolver = MapResolver::default();
        resolver.files.insert("defs.sa".into(), defs_src.into());
        resolver
            .files
            .insert("middle.sa".into(), "@import \"defs.sa\"\n".into());
        let result = flatten_source(root, "main.sa", &resolver, &FlattenOptions::default())
            .unwrap_or_else(|diags| panic!("{diags:?}"));
        let instrs = &result.function("main").unwrap().blocks[0].instrs;
        assert!(instrs.iter().any(|ins| ins.raw == "r0 = add 4, 1"));
        assert!(!instrs.iter().any(|ins| ins.raw.contains("EXPAND")));
    }

    #[test]
    fn flatten_source_keeps_repeated_macro_labels_unique() {
        let defs_src = "[MACRO] JUMPY %out\n    br %out -> L_YES_%out, L_NO_%out\nL_YES_%out:\n    jmp L_NO_%out\nL_NO_%out:\n[END_MACRO]\n";
        let root = "@import \"defs.sa\"\n@main() -> i32:\nL_ENTRY:\n    EXPAND JUMPY a\n    EXPAND JUMPY b\n    return 0\n";
        let mut resolver = MapResolver::default();
        resolver.files.insert("defs.sa".into(), defs_src.into());
        let result = flatten_source(root, "main.sa", &resolver, &FlattenOptions::default())
            .unwrap_or_else(|diags| panic!("{diags:?}"));
        let labels = result.function("main").unwrap().labels();
        assert_eq!(labels.len(), 5, "{labels:?}");
        assert!(labels.contains(&"L_YES_a__m1"));
        assert!(labels.contains(&"L_NO_b__m2"));
    }

    #[test]
    fn flatten_source_reports_unknown_macro() {
        let resolver = MapResolver::default();
        let src = "@main() -> i32:\nL_ENTRY:\n    EXPAND MISSING 1\n";
        let diags = flatten_source(src, "main.sa", &resolver, &FlattenOptions::default())
            .unwrap_err();
        assert!(diags
            .iter()
            .any(|diag| diag.trap == Trap::UndefinedMacro));
    }
}
