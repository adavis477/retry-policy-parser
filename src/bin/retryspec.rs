//! Validates a retry policy file and reports whether it's well-formed.
//!
//! Usage: `retryspec <path>`
//!
//! Prints the canonical form on success and exits 0. On failure it prints
//! the parse error to stderr and exits 1, so it's usable as a pre-commit or
//! CI check on a policy file.

use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let path = match (args.next(), args.next()) {
        (Some(path), None) => path,
        _ => {
            eprintln!("usage: retryspec <path>");
            return ExitCode::FAILURE;
        }
    };

    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("{}: {err}", path.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };

    match retryspec::parse(&text) {
        Ok(policy) => {
            println!("{}", retryspec::pretty_print(&policy));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}: {err}", path.to_string_lossy());
            ExitCode::FAILURE
        }
    }
}
