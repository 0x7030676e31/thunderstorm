#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::sync::Arc;

use tokio::sync::Mutex;

mod invokes;
mod state;
mod stream;
mod actions;

pub type AppState = Arc<Mutex<state::State>>;

#[tokio::main]
async fn main() {
  if env::var("RUST_LOG").is_err() {
    env::set_var("RUST_LOG", "info");
  }

  pretty_env_logger::init();
  log::info!("Starting Thunderstorm Desktop v{}", env!("CARGO_PKG_VERSION"));

  let state = state::State::new();
  let state = Arc::new(Mutex::new(state));

  let mut app_state = state.lock().await;
  app_state.state = Some(state.clone());

  drop(app_state);
  let state_clone = state.clone();

  tauri::Builder::default()
    .setup(|app| {
      let handle = app.handle();
      tokio::spawn(async move {
        let mut state = state_clone.lock().await;
        state.app_handle = Some(handle);
      });

      Ok(())
    })
    .manage(state)
    .invoke_handler(tauri::generate_handler![
      invokes::get_files,
      invokes::add_file,
      invokes::search,
      invokes::delete_file,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
