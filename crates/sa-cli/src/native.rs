use sa_ir::{Diagnostic, SourceLoc, Trap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var("PATH").unwrap_or_default();
    for dir in paths.split(':') {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_zig() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ZIG") {
        if !path.is_empty() {
            let candidate = PathBuf::from(&path);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(path) = which("zig") {
        return Some(path);
    }
    let fixed = PathBuf::from("/tools/zig/zig");
    if fixed.is_file() {
        return Some(fixed);
    }
    None
}

fn failure(trap: Trap, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::new(trap, message, SourceLoc::unknown())]
}

pub fn link_executable(
    c_source: &str,
    out: &str,
    debug: bool,
    optimize: bool,
) -> Result<(), Vec<Diagnostic>> {
    let zig = match find_zig() {
        Some(path) => path,
        None => {
            return Err(failure(
                Trap::ImportResolutionFailed,
                "zig compiler not found (set ZIG or put zig on PATH)",
            ))
        }
    };
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("sars_build_exe_{}_{}.c", std::process::id(), n));
    if let Err(e) = std::fs::write(&tmp, c_source) {
        return Err(failure(
            Trap::RuntimeError,
            format!("cannot write generated C source: {e}"),
        ));
    }
    let mut cmd = Command::new(&zig);
    cmd.arg("cc").arg(&tmp).arg("-o").arg(out);
    if optimize {
        cmd.arg("-O2");
    } else {
        cmd.arg("-O0");
    }
    if debug {
        cmd.arg("-g");
    }
    let result = cmd.output();
    let _ = std::fs::remove_file(&tmp);
    let output = match result {
        Ok(o) => o,
        Err(e) => {
            return Err(failure(
                Trap::RuntimeError,
                format!("failed to run zig cc: {e}"),
            ))
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if stderr.trim().is_empty() {
            format!("zig cc exited with {}", output.status)
        } else {
            format!("zig cc failed:\n{}", stderr.trim_end())
        };
        return Err(failure(Trap::RuntimeError, message));
    }
    Ok(())
}
