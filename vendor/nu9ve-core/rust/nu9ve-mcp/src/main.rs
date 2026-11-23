//! `nu9ve mcp [--allow-actions]`, `nu9ve agent <command>` and `nu9ve --version`.

use std::process::ExitCode;

const USAGE: &str = "uso: nu9ve mcp [--allow-actions]\n     nu9ve agent <orden> (nu9ve agent help)\n     nu9ve --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let actions = match args.as_slice() {
        ["--version"] => {
            println!("nu9ve {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        ["agent", rest @ ..] => {
            return match nu9ve_mcp::agent::run(rest) {
                Ok(value) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value).unwrap_or_default()
                    );
                    ExitCode::SUCCESS
                }
                Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
                    eprintln!("{error}");
                    ExitCode::from(2)
                }
                Err(error) => {
                    eprintln!("nu9ve agent: {error}");
                    ExitCode::FAILURE
                }
            };
        }
        ["mcp"] => false,
        ["mcp", "--allow-actions"] => true,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let options = nu9ve_mcp::Options {
        dir: nu9ve_ipc::default_dir(),
        actions,
        version: env!("CARGO_PKG_VERSION"),
    };
    match nu9ve_mcp::run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("nu9ve mcp: {error}");
            ExitCode::FAILURE
        }
    }
}
