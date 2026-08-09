//! Shared fixtures for the Phase 3 cases.
//!
//! Every case drives the built binary as a subprocess, because Phase 3's
//! subject is a process: its exit code, its two streams, and how it behaves
//! with no terminal attached.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

/// Long enough for C-4.
pub const PASSWORD: &str = "a sufficiently long password";

/// No command may take longer than this. A case that would block on a prompt
/// fails here rather than hanging the suite.
pub const PATIENCE: Duration = Duration::from_secs(60);

/// A scratch area that removes itself.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "veil2-cli-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let scratch = Self(dir);
        scratch.write("password.txt", PASSWORD);
        scratch
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    /// The vault every case uses unless it needs a second one.
    pub fn vault(&self) -> PathBuf {
        self.path("Test.veil")
    }

    pub fn password_file(&self) -> PathBuf {
        self.path("password.txt")
    }

    /// Writes a file, creating any folders above it.
    pub fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
        path
    }

    pub fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.path(relative)).unwrap()
    }

    pub fn exists(&self, relative: &str) -> bool {
        self.path(relative).exists()
    }

    /// Creates the vault and adds each `(folder, name, content)`.
    pub fn with_files(&self, files: &[(&str, &str, &str)]) -> &Self {
        assert_eq!(self.veil(&["create", &self.vault_arg()]).code, 0);
        for (folder, name, content) in files {
            let source = self.write(&format!("sources/{name}"), content);
            let run = self.veil(&[
                "add",
                &self.vault_arg(),
                source.to_str().unwrap(),
                "--folder",
                folder,
            ]);
            assert_eq!(run.code, 0, "adding {name} failed: {}", run.err);
        }
        self
    }

    pub fn vault_arg(&self) -> String {
        self.vault().display().to_string()
    }

    /// Runs the binary with the password file supplied.
    pub fn veil(&self, args: &[&str]) -> Run {
        let mut all: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        all.push("--password-file".to_owned());
        all.push(self.password_file().display().to_string());
        run(&all.iter().map(String::as_str).collect::<Vec<_>>())
    }

    /// Every entry file in the vault, in order.
    pub fn entry_files(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(self.vault().join("entries"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        paths
    }

    /// Flips every byte of one entry's file, which is total damage to it —
    /// and to nothing else, under one-file-per-entry storage.
    pub fn ruin(&self, entry_file: &Path) {
        let mut bytes = std::fs::read(entry_file).unwrap();
        for byte in &mut bytes {
            *byte ^= 0xFF;
        }
        std::fs::write(entry_file, bytes).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What one invocation produced.
pub struct Run {
    pub code: i32,
    pub out: String,
    pub err: String,
}

impl Run {
    /// Both streams, for the audits that must cover everything said.
    pub fn everything(&self) -> String {
        format!("{}\n{}", self.out, self.err)
    }
}

/// Runs the binary with no password supplied and a hermetic environment.
pub fn run(args: &[&str]) -> Run {
    finish(
        assert_cmd::Command::cargo_bin("veil")
            .unwrap()
            .args(args)
            .env_remove("VEIL_PASSWORD")
            .env_remove("VEIL_NEW_PASSWORD")
            .timeout(PATIENCE)
            .output()
            .unwrap(),
    )
}

/// Runs the binary with environment variables set.
pub fn run_with_env(args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut command = assert_cmd::Command::cargo_bin("veil").unwrap();
    command
        .args(args)
        .env_remove("VEIL_PASSWORD")
        .env_remove("VEIL_NEW_PASSWORD")
        .timeout(PATIENCE);
    for (key, value) in env {
        command.env(key, value);
    }
    finish(command.output().unwrap())
}

fn finish(output: Output) -> Run {
    Run {
        code: output.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&output.stdout).into_owned(),
        err: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}
