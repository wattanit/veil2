//! Where a password comes from (Spec §5.2; HC-2, Design §3.4).
//!
//! Never from a command-line argument. There is no option that takes one, so
//! there is nothing to discourage — arguments are visible in process listings
//! and shell history, and an option that exists is one `history | grep` away
//! from being the disclosure.

use std::io::IsTerminal;
use std::path::Path;

use veil_core::crypto::Password;

use crate::failure::{Failure, Run};

/// The environment variable holding a vault's password.
pub const PASSWORD_ENV: &str = "VEIL_PASSWORD";

/// The environment variable holding a new password, for a change.
pub const NEW_PASSWORD_ENV: &str = "VEIL_NEW_PASSWORD";

/// Resolves a password from a file, the environment, or a terminal — in that
/// order, and no further.
///
/// A non-interactive invocation with none of them fails immediately naming what
/// was missing, rather than blocking on a prompt nobody can answer. That is the
/// difference between a scripted run that reports an error and one that hangs
/// until someone notices (Design §3.4).
pub fn resolve(file: Option<&Path>, env: &str, prompt: &str, confirm: bool) -> Run<Password> {
    if let Some(path) = file {
        return from_file(path);
    }
    if let Ok(value) = std::env::var(env) {
        return Ok(Password::new(value));
    }
    if std::io::stdin().is_terminal() {
        return from_terminal(prompt, confirm);
    }
    Err(Failure::NoPassword(format!(
        "a password is needed and there is no terminal to ask on. \
         Give one with --password-file, or set {env}"
    )))
}

/// Reads a password file, dropping one trailing line ending.
///
/// Exactly one, and nothing else: a file written by `echo` ends in a newline
/// and that is the common case, but trimming every trailing space would
/// silently change a password that legitimately ends in one — and a password
/// its owner cannot reproduce is HC-7 arriving by accident.
fn from_file(path: &Path) -> Run<Password> {
    let bytes = std::fs::read(path).map_err(|e| {
        Failure::Usage(format!(
            "cannot read the password file {}: {e}",
            path.display()
        ))
    })?;

    let trimmed = match bytes.as_slice() {
        [head @ .., b'\r', b'\n'] | [head @ .., b'\n'] => head,
        whole => whole,
    };

    let text = std::str::from_utf8(trimmed)
        .map_err(|_| Failure::Usage(format!("the password file {} is not text", path.display())))?;
    Ok(Password::new(text.to_owned()))
}

/// Asks on the terminal, without echoing.
fn from_terminal(prompt: &str, confirm: bool) -> Run<Password> {
    let first = rpassword::prompt_password(format!("{prompt}: ")).map_err(|e| {
        Failure::Other(anyhow::Error::new(e).context("cannot read from the terminal"))
    })?;

    if confirm {
        let again = rpassword::prompt_password("Type it again: ").map_err(|e| {
            Failure::Other(anyhow::Error::new(e).context("cannot read from the terminal"))
        })?;
        if first != again {
            return Err(Failure::Usage(
                "those two do not match; nothing was changed".to_owned(),
            ));
        }
    }
    Ok(Password::new(first))
}
