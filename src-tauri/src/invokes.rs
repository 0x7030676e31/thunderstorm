use crate::AppState;

use tauri::State;

#[tauri::command]
pub async fn get_files(state: State<'_, AppState>) -> Result<String, ()> {
  let state = state.read().await;
  Ok(serde_json::to_string(&state.files).unwrap())
}

#[tauri::command]
pub async fn upload_files(state: State<'_, AppState>, files: Vec<String>) -> Result<(), ()> {
  let mut state = state.write().await;
  log::debug!("Adding files: {:?}", files);
  state.extend_queue(files);
  Ok(())
}