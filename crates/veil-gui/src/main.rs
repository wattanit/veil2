//! Entry point. Everything lives in `veil_gui_lib` so it is reachable from
//! an integration test, which cannot link against a `[[bin]]` target.

// Prevents an additional console window on Windows in release, harmless
// elsewhere; kept for when a Windows release is ever built (Requirements
// §2.2 — not yet, but this line costs nothing to have ready).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    veil_gui_lib::run();
}
