pub mod emit;
pub mod runtime;

#[cfg(test)]
mod tests;

use sa_ir::{Diagnostic, Program};

#[derive(Debug, Clone)]
pub struct COptions {
    pub debug: bool,
    pub optimize: bool,
    pub target: Option<String>,
    pub heap_size: usize,
}

impl Default for COptions {
    fn default() -> Self {
        Self {
            debug: false,
            optimize: true,
            target: None,
            heap_size: 1 << 24,
        }
    }
}

pub fn emit_c(program: &Program, opts: &COptions) -> Result<String, Vec<Diagnostic>> {
    emit::emit(program, opts)
}
