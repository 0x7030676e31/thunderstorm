use crate::AppState;

use tauri::State;
use serde::Serialize;

#[tauri::command]
pub async fn get_files(state: State<'_, AppState>) -> Result<String, ()> {
  let state = state.read().await;
  Ok(serde_json::to_string(&state.files).unwrap())
}

#[derive(Serialize)]
pub struct Settings<'a> {
  token: &'a String,
  channel: &'a String,
  guild: &'a String,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<String, ()> {
  let state = state.read().await;
  let settings = Settings {
    token: &state.token,
    channel: &state.channel_id,
    guild: &state.guild_id,
  };

  Ok(serde_json::to_string(&settings).unwrap())
}

#[tauri::command]
pub async fn upload_files(state: State<'_, AppState>, files: Vec<String>) -> Result<(), ()> {
  let mut state = state.write().await;
  log::debug!("Adding files: {:?}", files);
  state.extend_queue(files);
  Ok(())
}
