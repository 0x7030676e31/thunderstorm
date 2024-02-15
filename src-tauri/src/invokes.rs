use crate::state::File;
use crate::AppState;

use std::{fs, time};

use tauri::State;

#[tauri::command]
pub async fn get_state(state: State<'_, AppState>) -> Result<String, ()> {
  let state = state.lock().await;
  Ok(serde_json::to_string(&*state).unwrap())
}

#[tauri::command]
pub async fn set_state(state: State<'_, AppState>, token: String, aes_key: String, storage_channel: String) -> Result<(), ()> {
  let mut state = state.lock().await;
  state.token = Some(token);
  state.aes_key = Some(aes_key);
  state.storage_channel = Some(storage_channel);

  log::info!("Updated state");
  state.write();

  Ok(())
}

#[tauri::command]
pub async fn add_file(state: State<'_, AppState>, file: String) -> Result<String, ()> {
  let mut state = state.lock().await;
  let file_name = file.split("/").last().unwrap().to_string();
  let file = fs::metadata(file).unwrap();

  let now = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_millis() as u64;
  let file = File {
    name: file_name,
    size: file.len(),
    clusters: vec![],
    modified: now,
    created: now,
  };

  let data = serde_json::to_string(&file).unwrap();
  state.files.push(file);

  log::info!("Added file to state");
  state.write();

  Ok(data)
}
