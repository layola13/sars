mod buf;
pub mod dts;
mod emit;
mod ident;
mod lower;
mod npm;
mod runtime;
mod shims;
pub mod unbound;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsFormat {
    Esm,
    Cjs,
}

#[derive(Debug, Clone)]
pub struct JsOptions {
    pub format: JsFormat,
    pub debug: bool,
    pub entry: Option<String>,
    pub memory_pages: u32,
}

impl Default for JsOptions {
    fn default() -> Self {
        Self {
            format: JsFormat::Esm,
            debug: false,
            entry: None,
            memory_pages: 256,
        }
    }
}

pub fn emit_js(program: &sa_ir::Program, opts: &JsOptions) -> Result<String, Vec<sa_ir::Diagnostic>> {
    emit::emit(program, opts)
}
