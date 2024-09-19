#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![feature(let_chains)]

use std::env;
use std::sync::Arc;

use tokio::sync::RwLock;

mod api;
mod invokes;
mod io;
mod levenshtein;
mod state;
mod utils;

pub use state::{errors, model};
type AppState = Arc<RwLock<model::State>>;

#[tokio::main]
async fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    pretty_env_logger::init();
    log::info!(
        "Starting Thunderstorm Desktop v{}",
        env!("CARGO_PKG_VERSION")
    );

    let state = model::State::new();
    let state = Arc::new(RwLock::new(state));

    let mut app_state = state.write().await;
    app_state.rt.this = &state;
    drop(app_state);

    let state2 = state.clone();
    tauri::Builder::default()
        .setup(|app| {
            let handle = Arc::new(app.handle());
            tokio::spawn(async move {
                let mut app_state = state2.write().await;
                app_state.rt.app_handle = Arc::into_raw(handle) as *const _;
            });

            Ok(())
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            invokes::get_files,
            invokes::download_files,
            invokes::delete_files,
            invokes::get_settings,
            invokes::upload_files,
            invokes::set_settings,
            invokes::cancel,
            invokes::query,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
