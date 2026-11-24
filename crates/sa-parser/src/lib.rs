mod parse_header;
mod parse_instr;
mod parser;

pub use parser::{parse_instruction_line, parse_program, parse_source, parse_source_with_target};
