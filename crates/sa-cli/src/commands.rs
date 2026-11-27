use crate::cli::{Command, OutputFormat};
use crate::{json, native, pipeline};
use sa_interp::{InterpOptions, Interpreter};
use sa_ir::Diagnostic;
use std::io::Write;
use std::path::Path;

pub fn dispatch(command: Command) -> i32 {
    match command {
        Command::Help => {
            println!("{}", crate::cli::USAGE);
            0
        }
        Command::Version => {
            println!("sa {}", sa_ir::VERSION);
            0
        }
        Command::Check {
            file,
            json: as_json,
            no_verify,
            regstate,
        } => check(&file, as_json, no_verify, regstate),
        Command::Run { file, trace, gas } => run(&file, trace, gas),
        Command::BuildJs {
            file,
            out,
            format,
            debug,
            entry,
            emit_dts,
        } => build_js(&file, out, format, debug, entry, emit_dts),
        Command::BuildWasm { file, out, entry } => build_wasm(&file, out, entry),
        Command::BuildExe {
            file,
            out,
            debug,
            optimize,
            target,
            entry,
        } => build_exe(&file, out, debug, optimize, target, entry),
        Command::DumpIr { file } => dump_ir(&file),
        Command::DumpC { file } => dump_c(&file),
        Command::CompileTokens { file } => compile_tokens(&file),
    }
}

fn print_diagnostics(diags: &[Diagnostic], file: &str, as_json: bool) {
    if as_json {
        eprintln!("{}", json::diagnostics_json(diags));
        return;
    }
    for d in diags {
        let name = if d.loc.file.is_empty() {
            file
        } else {
            d.loc.file.as_str()
        };
        eprintln!(
            "{}:{}:{} [E{}] {}: {}",
            name,
            d.loc.line,
            d.loc.column,
            d.code(),
            d.trap,
            d.message
        );
        if let Some(hint) = &d.hint {
            eprintln!("  hint: {hint}");
        }
    }
}

fn fail(diags: &[Diagnostic], file: &str, as_json: bool) -> i32 {
    print_diagnostics(diags, file, as_json);
    1
}

fn check(file: &str, as_json: bool, no_verify: bool, regstate: bool) -> i32 {
    let opts = sa_verify::VerifyOptions {
        use_regstate: regstate,
        ..sa_verify::VerifyOptions::default()
    };
    match pipeline::compile_with_options(file, !no_verify, opts) {
        Ok(compilation) => {
            let functions = compilation.program.function_count();
            let instructions = compilation.program.instruction_count();
            let gas = pipeline::gas_estimate(&compilation.program);
            if as_json {
                println!("{}", json::check_json(functions, instructions, gas));
            } else {
                println!(
                    "OK: {functions} functions, {instructions} instructions, gas used {gas}"
                );
            }
            0
        }
        Err(diags) => fail(&diags, file, as_json),
    }
}

fn run(file: &str, trace: bool, gas_limit: u64) -> i32 {
    let compilation = match pipeline::compile(file, true) {
        Ok(c) => c,
        Err(diags) => return fail(&diags, file, false),
    };
    let opts = InterpOptions {
        max_steps: InterpOptions::default().max_steps,
        trace,
        gas_limit,
    };
    let mut interpreter = Interpreter::new(&compilation.program, opts);
    let result = interpreter.run();
    if trace {
        for line in interpreter.trace_log() {
            eprintln!("{line}");
        }
    }
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    if lock
        .write_all(interpreter.last_output())
        .and_then(|_| lock.flush())
        .is_err()
    {
        eprintln!("sa: failed to write program output");
        return 1;
    }
    match result {
        Ok(code) => code.clamp(0, 255),
        Err(e) => {
            eprintln!("RuntimeTrap: {e}");
            1
        }
    }
}

fn default_output(file: &str, extension: &str) -> String {
    Path::new(file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("out")
        .to_string()
        + extension
}

fn write_output(path: &str, bytes: &[u8]) -> i32 {
    match std::fs::write(path, bytes) {
        Ok(()) => {
            println!("{path}");
            0
        }
        Err(e) => {
            eprintln!("sa: cannot write {path}: {e}");
            1
        }
    }
}

fn build_js(
    file: &str,
    out: Option<String>,
    format: OutputFormat,
    debug: bool,
    entry: Option<String>,
    emit_dts: bool,
) -> i32 {
    let compilation = match pipeline::compile(file, true) {
        Ok(c) => c,
        Err(diags) => return fail(&diags, file, false),
    };
    let js_format = match format {
        OutputFormat::Esm => sa_emit_js::JsFormat::Esm,
        OutputFormat::Cjs => sa_emit_js::JsFormat::Cjs,
    };
    let opts = sa_emit_js::JsOptions {
        format: js_format,
        debug,
        entry,
        memory_pages: 256,
    };
    match sa_emit_js::emit_js(&compilation.program, &opts) {
        Ok(code) => {
            let ext = if format == OutputFormat::Cjs {
                ".cjs"
            } else {
                ".mjs"
            };
            let js_path = out.unwrap_or_else(|| default_output(file, ext));
            let dts = if emit_dts {
                match sa_emit_js::dts::emit_dts(&compilation.program, file) {
                    Ok(text) => Some(text),
                    Err(diags) => return fail(&diags, file, false),
                }
            } else {
                None
            };
            let rc = write_output(&js_path, code.as_bytes());
            if rc != 0 {
                return rc;
            }
            match dts {
                Some(text) => {
                    let dts_path = sa_emit_js::dts::dts_path_for_js_path(&js_path);
                    write_output(&dts_path, text.as_bytes())
                }
                None => rc,
            }
        }
        Err(diags) => fail(&diags, file, false),
    }
}

fn build_wasm(file: &str, out: Option<String>, entry: Option<String>) -> i32 {
    let _ = entry;
    let compilation = match pipeline::compile(file, true) {
        Ok(c) => c,
        Err(diags) => return fail(&diags, file, false),
    };
    if !compilation.program.npm_binds.is_empty() {
        let diags = vec![Diagnostic::new(
            sa_ir::Trap::UnsupportedInstruction,
            "build-wasm rejects #npm_bind programs (D22): npm bindings are JS-only",
            compilation.program.npm_binds[0].loc.clone(),
        )];
        return fail(&diags, file, false);
    }
    match sa_emit_wasm::emit_wasm(&compilation.program, &sa_emit_wasm::WasmOptions::default()) {
        Ok(bytes) => write_output(
            &out.unwrap_or_else(|| default_output(file, ".wasm")),
            &bytes,
        ),
        Err(diags) => fail(&diags, file, false),
    }
}

fn build_exe(
    file: &str,
    out: Option<String>,
    debug: bool,
    optimize: bool,
    target: Option<String>,
    entry: Option<String>,
) -> i32 {
    let compilation = match pipeline::compile(file, true) {
        Ok(c) => c,
        Err(diags) => return fail(&diags, file, false),
    };
    let mut diags = Vec::new();
    if let Some(name) = &entry {
        if name != "main" {
            diags.push(Diagnostic::new(
                sa_ir::Trap::UnsupportedInstruction,
                format!("--entry {name} is not supported by build-exe (first increment); main only"),
                sa_ir::SourceLoc::unknown(),
            ));
        }
    }
    if !diags.is_empty() {
        return fail(&diags, file, false);
    }
    let opts = sa_emit_c::COptions {
        debug,
        optimize,
        target,
        heap_size: 1 << 24,
    };
    let source = match sa_emit_c::emit_c(&compilation.program, &opts) {
        Ok(source) => source,
        Err(diags) => return fail(&diags, file, false),
    };
    let out_path = out.unwrap_or_else(|| default_output(file, ""));
    match native::link_executable(&source, &out_path, debug, optimize) {
        Ok(()) => {
            println!("{out_path}");
            0
        }
        Err(diags) => fail(&diags, file, false),
    }
}

fn dump_ir(file: &str) -> i32 {
    match pipeline::compile(file, true) {
        Ok(compilation) => {
            println!("{}", compilation.program.to_text());
            0
        }
        Err(diags) => fail(&diags, file, false),
    }
}

fn dump_c(file: &str) -> i32 {
    let compilation = match pipeline::compile(file, true) {
        Ok(c) => c,
        Err(diags) => return fail(&diags, file, false),
    };
    match sa_emit_c::emit_c(&compilation.program, &sa_emit_c::COptions::default()) {
        Ok(source) => {
            print!("{source}");
            0
        }
        Err(diags) => fail(&diags, file, false),
    }
}

fn compile_tokens(file: &str) -> i32 {
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sa: cannot read {file}: {e}");
            return 1;
        }
    };
    let tokens = match sa_lexer::tokenize(&source, file) {
        Ok(tokens) => tokens.len(),
        Err(d) => return fail(std::slice::from_ref(&d), file, false),
    };
    let compilation = match pipeline::compile(file, true) {
        Ok(c) => c,
        Err(diags) => return fail(&diags, file, false),
    };
    println!(
        "{}",
        json::tokens_json(
            tokens,
            compilation.program.function_count(),
            compilation.program.instruction_count()
        )
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse_args;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn defaults_outputs() {
        assert_eq!(default_output("dir/main.sa", ".mjs"), "main.mjs");
        assert_eq!(default_output("x", ".cjs"), "x.cjs");
        assert_eq!(default_output("/a/b.sa", ".wasm"), "b.wasm");
    }

    #[test]
    fn dispatches_help_and_version() {
        assert_eq!(dispatch(parse_args(&args(&["help"])).unwrap()), 0);
        assert_eq!(dispatch(parse_args(&args(&["--version"])).unwrap()), 0);
    }

    #[test]
    fn missing_file_reports_failure() {
        let cmd = parse_args(&args(&["check", "/nonexistent/file.sa"])).unwrap();
        assert_eq!(dispatch(cmd), 1);
    }

    #[test]
    fn build_wasm_rejects_npm_bind() {
        let path = std::env::temp_dir().join(format!("sars_npm_bind_{}.sa", std::process::id()));
        let out = path.with_extension("wasm");
        let _ = std::fs::remove_file(&out);
        let source = "@extern npm_totalmem() -> u64\n\
                      #npm_bind npm_totalmem \"node:os#totalmem\"\n\
                      @main() -> i32:\n\
                      L_ENTRY:\n\
                          return 0\n";
        std::fs::write(&path, source).unwrap();
        let cmd = parse_args(&args(&[
            "build-wasm",
            path.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]))
        .unwrap();
        assert_eq!(dispatch(cmd), 1);
        assert!(!out.exists());
        let _ = std::fs::remove_file(&path);
    }
}
