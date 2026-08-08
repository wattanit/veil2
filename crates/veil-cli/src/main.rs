//! `veil` — the command-line application (Spec §5.2).
//!
//! A peer of the graphical application, not a debug tool (A-4): every capability
//! `veil-core` has is reachable from here.

mod cli;
mod failure;
mod output;
mod password;
mod progress;
mod report;
mod run;

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Format};
use crate::failure::Failure;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run::dispatch(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            complain(cli.format, &failure);
            ExitCode::from(failure.code())
        }
    }
}

/// Reports a failure on standard error, in whichever form was asked for.
///
/// On standard error rather than standard output, even in JSON: standard output
/// carries results, and a script reading it should find valid output or nothing
/// at all — never an error object where a listing was expected.
fn complain(format: Format, failure: &Failure) {
    let mut err = std::io::stderr().lock();
    let _ = match format {
        Format::Table => writeln!(err, "{}", failure.message()),
        Format::Json => writeln!(
            err,
            "{}",
            serde_json::json!({
                "error": failure.message(),
                "exit_code": failure.code(),
            })
        ),
    };
}
