//! Tauri shell for ProtonTrainer.
//!
//! This crate only adapts the workspace crates to Tauri commands. It must not
//! contain business logic: anything testable without a window belongs in a
//! `pt-*` crate.

use serde::Serialize;

/// Basic information about the running application.
///
/// Exists in Phase 0 to prove that the frontend/backend IPC is wired up.
#[derive(Debug, Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}

#[tauri::command]
fn get_app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Builds and runs the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_app_info])
        .run(tauri::generate_context!())
        .expect("error while running ProtonTrainer");
}
