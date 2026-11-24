pub mod capability;
pub mod instr;
pub mod program;
pub mod trap;
pub mod types;

pub use capability::{Capability, Gas};
pub use instr::{Address, CallArg, Instruction, InstructionKind, OpKind, Operand};
pub use program::{Block, DefDecl, Function, ImportDecl, LocDecl, Program, SourceLoc};
pub use trap::{Diagnostic, Trap};
pub use types::{
    AtomicOrdering, AtomicRmwOp, CapPrefix, ConstDecl, ConstValue, FunctionKind, ParamSpec,
    PrimType, Signature,
};

pub const VERSION: &str = "0.1.0";
