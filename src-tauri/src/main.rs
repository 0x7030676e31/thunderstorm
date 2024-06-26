#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::env;

use tokio::sync::RwLock;

mod invokes;
mod state;
mod reader;
mod api;

type AppState = Arc<RwLock<state::State>>;

#[tokio::main]
async fn main() {
  if env::var("RUST_LOG").is_err() {
    env::set_var("RUST_LOG", "info");
  }

  pretty_env_logger::init();
  log::info!("Starting Thunderstorm Desktop v{}", env!("CARGO_PKG_VERSION"));

  let state = state::State::new();
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
      invokes::get_settings,
      invokes::upload_files,
      invokes::set_settings,
      invokes::cancel,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
