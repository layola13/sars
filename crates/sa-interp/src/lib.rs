use sa_ir::Trap;
use std::fmt;

mod exec;
mod memory;
mod value;

pub use exec::Interpreter;
pub use memory::Memory;
pub use value::Value;

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub trap: Trap,
    pub message: String,
}

impl RuntimeError {
    pub fn new(trap: Trap, message: impl Into<String>) -> Self {
        Self {
            trap,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.trap.name(), self.message)
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Clone)]
pub struct InterpOptions {
    pub max_steps: u64,
    pub trace: bool,
    pub gas_limit: u64,
}

impl Default for InterpOptions {
    fn default() -> Self {
        Self {
            max_steps: 10_000_000,
            trace: false,
            gas_limit: 100_000_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let o = InterpOptions::default();
        assert_eq!(o.max_steps, 10_000_000);
        assert_eq!(o.gas_limit, 100_000_000);
        assert!(!o.trace);
    }

    #[test]
    fn error_display_and_source() {
        let e = RuntimeError::new(Trap::RuntimeError, "boom");
        assert_eq!(e.to_string(), "RuntimeError: boom");
        let _: &dyn std::error::Error = &e;
    }
}
