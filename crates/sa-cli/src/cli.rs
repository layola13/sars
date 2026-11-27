use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Esm,
    Cjs,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Check {
        file: String,
        json: bool,
        no_verify: bool,
        regstate: bool,
    },
    Run {
        file: String,
        trace: bool,
        gas: u64,
    },
    BuildJs {
        file: String,
        out: Option<String>,
        format: OutputFormat,
        debug: bool,
        entry: Option<String>,
        emit_dts: bool,
    },
    BuildWasm {
        file: String,
        out: Option<String>,
        entry: Option<String>,
    },
    BuildExe {
        file: String,
        out: Option<String>,
        debug: bool,
        optimize: bool,
        target: Option<String>,
        entry: Option<String>,
    },
    DumpIr {
        file: String,
    },
    DumpC {
        file: String,
    },
    CompileTokens {
        file: String,
    },
    Help,
    Version,
}

pub const USAGE: &str = concat!(
    "Usage: sa <command> [options]\n\n",
    "Commands:\n",
    "  check <file> [--json] [--no-verify] [--regstate]\n",
    "  run <file> [--trace] [--gas N]\n",
    "  build-js <file> [-o <out>] [--format esm|cjs] [-g|--no-debug] [--entry NAME] [--emit-dts]\n",
    "  build-wasm <file> [-o <out>] [--entry NAME]\n",
    "  build-exe <file> [-o <out>] [-g|--no-debug] [-O0|-O2] [--target TRIPLE] [--entry NAME]\n",
    "  dump-ir <file>\n",
    "  dump-c <file>                 Print generated native C source without linking\n",
    "  compile-tokens <file>\n",
    "  help | --help | --version\n"
);

#[derive(Debug, Clone, PartialEq)]
pub struct CliError {
    pub message: String,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

fn error(message: impl Into<String>) -> CliError {
    CliError {
        message: message.into(),
    }
}

fn take_value(rest: &[String], index: &mut usize, flag: &str) -> Result<String, CliError> {
    if *index >= rest.len() {
        return Err(error(format!("{flag} requires a value")));
    }
    let value = rest[*index].clone();
    *index += 1;
    Ok(value)
}

fn inline_value(arg: &str, flag: &str) -> Option<String> {
    arg.strip_prefix(flag).map(|v| v.to_string())
}

fn set_file(slot: &mut Option<String>, value: String) -> Result<(), CliError> {
    if slot.is_some() {
        return Err(error("unexpected extra argument"));
    }
    *slot = Some(value);
    Ok(())
}

fn require_file(slot: Option<String>, command: &str) -> Result<String, CliError> {
    slot.ok_or_else(|| error(format!("{command} requires a file")))
}

pub fn parse_args(args: &[String]) -> Result<Command, CliError> {
    let first = args.first().ok_or_else(|| error("missing command"))?;
    let rest = &args[1..];
    match first.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "--version" | "-V" => Ok(Command::Version),
        "check" => {
            let mut file = None;
            let mut json = false;
            let mut no_verify = false;
            let mut regstate = false;
            let mut i = 0;
            while i < rest.len() {
                let arg = rest[i].as_str();
                i += 1;
                match arg {
                    "--json" => json = true,
                    "--no-verify" => no_verify = true,
                    "--regstate" => regstate = true,
                    s if s.starts_with('-') => {
                        return Err(error(format!("unknown option: {s}")));
                    }
                    s => set_file(&mut file, s.to_string())?,
                }
            }
            Ok(Command::Check {
                file: require_file(file, "check")?,
                json,
                no_verify,
                regstate,
            })
        }
        "run" => {
            let mut file = None;
            let mut trace = false;
            let mut gas = 100_000_000u64;
            let mut i = 0;
            while i < rest.len() {
                let arg = rest[i].as_str();
                i += 1;
                match arg {
                    "--trace" => trace = true,
                    "--gas" => {
                        let raw = take_value(rest, &mut i, "--gas")?;
                        gas = raw
                            .parse::<u64>()
                            .map_err(|_| error(format!("invalid gas value: {raw}")))?;
                    }
                    s if s.starts_with("--gas=") => {
                        let raw = &s[6..];
                        gas = raw
                            .parse::<u64>()
                            .map_err(|_| error(format!("invalid gas value: {raw}")))?;
                    }
                    s if s.starts_with('-') => {
                        return Err(error(format!("unknown option: {s}")));
                    }
                    s => set_file(&mut file, s.to_string())?,
                }
            }
            Ok(Command::Run {
                file: require_file(file, "run")?,
                trace,
                gas,
            })
        }
        "build-js" => {
            let mut file = None;
            let mut out = None;
            let mut format = OutputFormat::default();
            let mut debug = true;
            let mut entry = None;
            let mut emit_dts = false;
            let mut i = 0;
            while i < rest.len() {
                let arg = rest[i].as_str();
                i += 1;
                match arg {
                    "-o" | "--output" => out = Some(take_value(rest, &mut i, arg)?),
                    "--format" => {
                        let raw = take_value(rest, &mut i, "--format")?;
                        format = parse_format(&raw)?;
                    }
                    "-g" | "--no-debug" => debug = false,
                    "--entry" => entry = Some(take_value(rest, &mut i, "--entry")?),
                    "--emit-dts" => emit_dts = true,
                    s if s.starts_with("--format=") => format = parse_format(&s[9..])?,
                    s if s.starts_with("--entry=") => entry = Some(inline_value(s, "--entry=").unwrap()),
                    s if s.starts_with('-') => {
                        return Err(error(format!("unknown option: {s}")));
                    }
                    s => set_file(&mut file, s.to_string())?,
                }
            }
            Ok(Command::BuildJs {
                file: require_file(file, "build-js")?,
                out,
                format,
                debug,
                entry,
                emit_dts,
            })
        }
        "build-wasm" => {
            let mut file = None;
            let mut out = None;
            let mut entry = None;
            let mut i = 0;
            while i < rest.len() {
                let arg = rest[i].as_str();
                i += 1;
                match arg {
                    "-o" | "--output" => out = Some(take_value(rest, &mut i, arg)?),
                    "--entry" => entry = Some(take_value(rest, &mut i, "--entry")?),
                    s if s.starts_with("--entry=") => entry = Some(inline_value(s, "--entry=").unwrap()),
                    s if s.starts_with('-') => {
                        return Err(error(format!("unknown option: {s}")));
                    }
                    s => set_file(&mut file, s.to_string())?,
                }
            }
            Ok(Command::BuildWasm {
                file: require_file(file, "build-wasm")?,
                out,
                entry,
            })
        }
        "build-exe" | "build" => parse_build_exe(rest, first),
        "dump-ir" => {
            let mut file = None;
            for arg in rest {
                if arg.starts_with('-') {
                    return Err(error(format!("unknown option: {arg}")));
                }
                set_file(&mut file, arg.clone())?;
            }
            Ok(Command::DumpIr {
                file: require_file(file, "dump-ir")?,
            })
        }
        "dump-c" => {
            let mut file = None;
            for arg in rest {
                if arg.starts_with('-') {
                    return Err(error(format!("unknown option: {arg}")));
                }
                set_file(&mut file, arg.clone())?;
            }
            Ok(Command::DumpC {
                file: require_file(file, "dump-c")?,
            })
        }
        "compile-tokens" => {
            let mut file = None;
            for arg in rest {
                if arg.starts_with('-') {
                    return Err(error(format!("unknown option: {arg}")));
                }
                set_file(&mut file, arg.clone())?;
            }
            Ok(Command::CompileTokens {
                file: require_file(file, "compile-tokens")?,
            })
        }
        other => Err(error(format!("unknown command: {other}"))),
    }
}

fn parse_build_exe(rest: &[String], command: &str) -> Result<Command, CliError> {
    let mut file = None;
    let mut out = None;
    let mut debug = true;
    let mut optimize = true;
    let mut target = None;
    let mut entry = None;
    let mut i = 0;
    while i < rest.len() {
        let arg = rest[i].as_str();
        i += 1;
        match arg {
            "-o" | "--output" => out = Some(take_value(rest, &mut i, arg)?),
            "-g" => debug = true,
            "--no-debug" => debug = false,
            "-O0" => optimize = false,
            "-O2" | "--release-small" | "--release-fast" => optimize = true,
            "--target" => target = Some(take_value(rest, &mut i, "--target")?),
            "--entry" => entry = Some(take_value(rest, &mut i, "--entry")?),
            s if s.starts_with("--target=") => target = Some(inline_value(s, "--target=").unwrap()),
            s if s.starts_with("--entry=") => entry = Some(inline_value(s, "--entry=").unwrap()),
            "--incremental" | "--profile" | "-p" | "--package" => {
                return Err(error(format!(
                    "{arg} is not supported by build-exe (first increment)"
                )));
            }
            s if s.starts_with('-') => {
                return Err(error(format!("unknown option: {s}")));
            }
            s => set_file(&mut file, s.to_string())?,
        }
    }
    Ok(Command::BuildExe {
        file: require_file(file, command)?,
        out,
        debug,
        optimize,
        target,
        entry,
    })
}

fn parse_format(raw: &str) -> Result<OutputFormat, CliError> {
    match raw {
        "esm" => Ok(OutputFormat::Esm),
        "cjs" => Ok(OutputFormat::Cjs),
        _ => Err(error(format!("invalid format: {raw}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn parses_check_flags_and_defaults() {
        assert_eq!(
            parse_args(&args(&["check", "a.sa"])).unwrap(),
            Command::Check {
                file: "a.sa".to_string(),
                json: false,
                no_verify: false,
                regstate: false
            }
        );
        assert_eq!(
            parse_args(&args(&["check", "a.sa", "--json", "--no-verify"])).unwrap(),
            Command::Check {
                file: "a.sa".to_string(),
                json: true,
                no_verify: true,
                regstate: false
            }
        );
        assert_eq!(
            parse_args(&args(&["check", "a.sa", "--regstate"])).unwrap(),
            Command::Check {
                file: "a.sa".to_string(),
                json: false,
                no_verify: false,
                regstate: true
            }
        );
    }

    #[test]
    fn parses_run_gas_and_trace() {
        assert_eq!(
            parse_args(&args(&["run", "a.sa"])).unwrap(),
            Command::Run {
                file: "a.sa".to_string(),
                trace: false,
                gas: 100_000_000
            }
        );
        assert_eq!(
            parse_args(&args(&["run", "a.sa", "--trace", "--gas", "42"])).unwrap(),
            Command::Run {
                file: "a.sa".to_string(),
                trace: true,
                gas: 42
            }
        );
        assert_eq!(
            parse_args(&args(&["run", "a.sa", "--gas=7"])).unwrap(),
            Command::Run {
                file: "a.sa".to_string(),
                trace: false,
                gas: 7
            }
        );
        assert!(parse_args(&args(&["run", "a.sa", "--gas", "x"])).is_err());
    }

    #[test]
    fn parses_build_flags() {
        assert_eq!(
            parse_args(&args(&["build-js", "a.sa"])).unwrap(),
            Command::BuildJs {
                file: "a.sa".to_string(),
                out: None,
                format: OutputFormat::Esm,
                debug: true,
                entry: None,
                emit_dts: false
            }
        );
        assert_eq!(
            parse_args(&args(&[
                "build-js", "a.sa", "-o", "x.js", "--format", "cjs", "-g", "--entry", "go"
            ]))
            .unwrap(),
            Command::BuildJs {
                file: "a.sa".to_string(),
                out: Some("x.js".to_string()),
                format: OutputFormat::Cjs,
                debug: false,
                entry: Some("go".to_string()),
                emit_dts: false
            }
        );
        assert_eq!(
            parse_args(&args(&["build-wasm", "a.sa", "--entry=main"])).unwrap(),
            Command::BuildWasm {
                file: "a.sa".to_string(),
                out: None,
                entry: Some("main".to_string())
            }
        );
    }

    #[test]
    fn parses_emit_dts_flag() {
        match parse_args(&args(&["build-js", "a.sa"])).unwrap() {
            Command::BuildJs { emit_dts, .. } => assert!(!emit_dts),
            other => panic!("unexpected command: {other:?}"),
        }
        assert_eq!(
            parse_args(&args(&["build-js", "a.sa", "--emit-dts"])).unwrap(),
            Command::BuildJs {
                file: "a.sa".to_string(),
                out: None,
                format: OutputFormat::Esm,
                debug: true,
                entry: None,
                emit_dts: true
            }
        );
        assert_eq!(
            parse_args(&args(&[
                "build-js", "a.sa", "-o", "x.mjs", "--format", "esm", "-g", "--entry", "go",
                "--emit-dts"
            ]))
            .unwrap(),
            Command::BuildJs {
                file: "a.sa".to_string(),
                out: Some("x.mjs".to_string()),
                format: OutputFormat::Esm,
                debug: false,
                entry: Some("go".to_string()),
                emit_dts: true
            }
        );
    }

    #[test]
    fn parses_build_exe_flags_and_alias() {
        assert_eq!(
            parse_args(&args(&["build-exe", "a.sa"])).unwrap(),
            Command::BuildExe {
                file: "a.sa".to_string(),
                out: None,
                debug: true,
                optimize: true,
                target: None,
                entry: None
            }
        );
        assert_eq!(
            parse_args(&args(&[
                "build", "a.sa", "-o", "a.out", "--no-debug", "-O0", "--target", "host"
            ]))
            .unwrap(),
            Command::BuildExe {
                file: "a.sa".to_string(),
                out: Some("a.out".to_string()),
                debug: false,
                optimize: false,
                target: Some("host".to_string()),
                entry: None
            }
        );
        assert!(parse_args(&args(&["build-exe", "a.sa", "--incremental"])).is_err());
    }

    #[test]
    fn parses_simple_commands() {
        assert_eq!(
            parse_args(&args(&["dump-ir", "a.sa"])).unwrap(),
            Command::DumpIr {
                file: "a.sa".to_string()
            }
        );
        assert_eq!(
            parse_args(&args(&["compile-tokens", "a.sa"])).unwrap(),
            Command::CompileTokens {
                file: "a.sa".to_string()
            }
        );
        assert_eq!(parse_args(&args(&["help"])).unwrap(), Command::Help);
        assert_eq!(parse_args(&args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse_args(&args(&["--version"])).unwrap(), Command::Version);
    }

    #[test]
    fn rejects_invalid_arguments() {
        assert!(parse_args(&args(&[])).is_err());
        assert!(parse_args(&args(&["frobnicate"])).is_err());
        assert!(parse_args(&args(&["check"])).is_err());
        assert!(parse_args(&args(&["check", "a.sa", "b.sa"])).is_err());
        assert!(parse_args(&args(&["run", "a.sa", "--wat"])).is_err());
        assert!(parse_args(&args(&["build-js", "a.sa", "--format", "umd"])).is_err());
        assert!(parse_args(&args(&["build-js", "a.sa", "-o"])).is_err());
    }
}
