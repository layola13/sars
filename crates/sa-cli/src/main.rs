mod cli;
mod commands;
mod json;
mod pipeline;

use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::parse_args(&args) {
        Ok(command) => exit(commands::dispatch(command)),
        Err(e) => {
            eprintln!("sa: {e}");
            eprintln!("{}", cli::USAGE);
            exit(2);
        }
    }
}
