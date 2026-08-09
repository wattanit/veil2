//! Application state: at most one vault open at a time (Design §3.1), and
//! the cancellation token for whichever operation is running, if any.
//!
//! The two live behind separate locks on purpose: cancelling has to reach
//! the token while a long operation is still holding the vault, and a
//! single lock covering both would make `cancel_operation` wait behind the
//! very operation it exists to interrupt (P5.1.e).

use std::sync::Mutex;

use veil_core::{Cancel, Vault};

/// Tauri managed state: `app.state::<AppState>()` reaches one of these from
/// any command.
#[derive(Default)]
pub struct AppState {
    vault: Mutex<Option<Vault>>,
    current_cancel: Mutex<Option<Cancel>>,
}

impl AppState {
    /// Replaces whatever vault was open, if any.
    pub fn set_vault(&self, vault: Vault) -> Result<(), String> {
        *self.vault.lock().map_err(poisoned)? = Some(vault);
        Ok(())
    }

    /// Drops the open vault, if any. Nothing is written.
    pub fn clear_vault(&self) -> Result<(), String> {
        *self.vault.lock().map_err(poisoned)? = None;
        Ok(())
    }

    /// Runs `f` against the open vault, or fails with "no vault open".
    pub fn with_vault<T>(&self, f: impl FnOnce(&Vault) -> Result<T, String>) -> Result<T, String> {
        let guard = self.vault.lock().map_err(poisoned)?;
        let vault = guard.as_ref().ok_or_else(|| "no vault open".to_owned())?;
        f(vault)
    }

    /// As [`with_vault`](Self::with_vault), for an operation that mutates
    /// the vault.
    pub fn with_vault_mut<T>(
        &self,
        f: impl FnOnce(&mut Vault) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self.vault.lock().map_err(poisoned)?;
        let vault = guard.as_mut().ok_or_else(|| "no vault open".to_owned())?;
        f(vault)
    }

    /// Starts a new cancellable operation, replacing whatever token was
    /// there before — Design §3.2's operation bar shows one operation at a
    /// time, so there is never more than one token to track.
    pub fn begin_cancellable(&self) -> Result<Cancel, String> {
        let cancel = Cancel::new();
        *self.current_cancel.lock().map_err(poisoned)? = Some(cancel.clone());
        Ok(cancel)
    }

    /// Clears the current operation's token once it finishes, cancelled or
    /// not — there is nothing left for `cancel_operation` to reach.
    pub fn end_cancellable(&self) -> Result<(), String> {
        *self.current_cancel.lock().map_err(poisoned)? = None;
        Ok(())
    }

    /// Cancels whichever operation is running, if any. A no-op if none is.
    pub fn cancel_current(&self) -> Result<(), String> {
        if let Some(cancel) = self.current_cancel.lock().map_err(poisoned)?.as_ref() {
            cancel.cancel();
        }
        Ok(())
    }
}

fn poisoned<T>(_: std::sync::PoisonError<T>) -> String {
    "vault state lock poisoned".to_owned()
}
