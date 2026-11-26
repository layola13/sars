use sa_emit_js::dts::{dts_path_for_js_path, emit_dts};
use sa_ir::{CapPrefix, Function, FunctionKind, ParamSpec, PrimType, Program, Signature, SourceLoc};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SMOKE_SOURCE: &str = "@export smoke_add(a: i32, b: i32) -> i32:\n\
L_ENTRY:\n\
    s = add a, b\n\
    return s\n";

fn smoke_program() -> Program {
    let mut sig = Signature::new("smoke_add");
    sig.kind = FunctionKind::Exported;
    sig.params = vec![
        ParamSpec::new("a", PrimType::I32, CapPrefix::ByValue),
        ParamSpec::new("b", PrimType::I32, CapPrefix::ByValue),
    ];
    sig.return_ty = PrimType::I32;
    let mut program = Program::new();
    program
        .functions
        .push(Function::new(sig, SourceLoc::unknown()));
    program
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sars_d3_{}_{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn sars() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sars"))
}

fn run(args: &[&str]) -> Output {
    sars().args(args).output().unwrap()
}

fn status_text(out: &Output) -> String {
    format!(
        "rc={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    )
}

fn sorted_lines(data: &[u8]) -> Vec<&[u8]> {
    let mut lines: Vec<&[u8]> = data.split(|&b| b == b'\n').collect();
    lines.sort();
    lines
}

#[test]
fn d3_emit_dts_writes_declaration_matching_dts_emitter() {
    let dir = temp_dir("matching");
    let source = dir.join("smoke.sa");
    fs::write(&source, SMOKE_SOURCE).unwrap();
    let source_s = source.to_str().unwrap();
    let js = dir.join("smoke.mjs");
    let js_s = js.to_str().unwrap();

    let out = run(&["build-js", source_s, "-o", js_s, "--emit-dts"]);
    assert!(out.status.success(), "{}", status_text(&out));

    let dts_path = dts_path_for_js_path(js_s);
    let written = fs::read_to_string(&dts_path).expect("companion .d.ts must be written");
    assert_eq!(
        written,
        emit_dts(&smoke_program(), source_s).unwrap(),
        ".d.ts must be exactly the J1 emitter output"
    );
    assert!(written.contains("export declare function smoke_add(a: number, b: number): number;"));
    assert!(written.contains(&format!("from {source_s}")));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn d3_default_off_writes_no_dts_and_js_unchanged() {
    let dir = temp_dir("default_off");
    let source = dir.join("smoke.sa");
    fs::write(&source, SMOKE_SOURCE).unwrap();
    let source_s = source.to_str().unwrap();
    let js_off = dir.join("off.mjs");
    let js_on = dir.join("on.mjs");

    let off = run(&["build-js", source_s, "-o", js_off.to_str().unwrap()]);
    assert!(off.status.success(), "{}", status_text(&off));
    let js_bytes = fs::read(&js_off).unwrap();
    assert!(!js_bytes.is_empty());
    assert!(
        !Path::new(&dts_path_for_js_path(js_off.to_str().unwrap())).exists(),
        "flag off must not write a .d.ts"
    );

    let on = run(&[
        "build-js",
        source_s,
        "-o",
        js_on.to_str().unwrap(),
        "--emit-dts",
    ]);
    assert!(on.status.success(), "{}", status_text(&on));
    assert_eq!(
        sorted_lines(&fs::read(&js_on).unwrap()),
        sorted_lines(&js_bytes),
        "JS emission must stay unchanged with the flag on"
    );
    assert!(Path::new(&dts_path_for_js_path(js_on.to_str().unwrap())).exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn d3_default_output_path_derives_companion_dts() {
    let dir = temp_dir("default_path");
    fs::write(dir.join("smoke.sa"), SMOKE_SOURCE).unwrap();

    let out = sars()
        .current_dir(&dir)
        .args(&["build-js", "smoke.sa", "--emit-dts"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", status_text(&out));
    assert!(dir.join("smoke.mjs").exists());
    let written = fs::read_to_string(dir.join("smoke.d.ts")).expect("smoke.d.ts must exist");
    assert_eq!(written, emit_dts(&smoke_program(), "smoke.sa").unwrap());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn d3_custom_cjs_output_path_derives_companion_dts() {
    let dir = temp_dir("custom_cjs");
    let source = dir.join("smoke.sa");
    fs::write(&source, SMOKE_SOURCE).unwrap();
    let source_s = source.to_str().unwrap();
    let js = dir.join("artifact.min.cjs");
    let js_s = js.to_str().unwrap();

    let out = run(&[
        "build-js", source_s, "-o", js_s, "--format", "cjs", "--emit-dts",
    ]);
    assert!(out.status.success(), "{}", status_text(&out));
    let dts_path = dts_path_for_js_path(js_s);
    assert_eq!(dts_path, dir.join("artifact.min.d.ts").to_str().unwrap());
    assert_eq!(
        fs::read_to_string(&dts_path).unwrap(),
        emit_dts(&smoke_program(), source_s).unwrap()
    );

    let _ = fs::remove_dir_all(&dir);
}
