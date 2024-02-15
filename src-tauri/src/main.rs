#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::sync::Arc;

use tokio::sync::Mutex;

mod invokes;
mod state;

pub type AppState = Arc<Mutex<state::State>>;

#[tokio::main]
async fn main() {
  if env::var("RUST_LOG").is_err() {
    env::set_var("RUST_LOG", "info");
  }

  pretty_env_logger::init();
  log::info!("Starting Thunderstorm Desktop v{}", env!("CARGO_PKG_VERSION"));

  let state = Arc::new(Mutex::new(state::State::new()));

  tauri::Builder::default()
    .manage(state)
    .invoke_handler(tauri::generate_handler![
      invokes::get_state,
      invokes::set_state,
      invokes::add_file,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
