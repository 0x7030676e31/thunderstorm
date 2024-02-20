use crate::AppState;

use textdistance::nstr::damerau_levenshtein;
use tauri::State;

#[tauri::command]
pub async fn get_files(state: State<'_, AppState>) -> Result<String, ()> {
  let state = state.lock().await;
  Ok(serde_json::to_string(&state.files).unwrap())
}

#[tauri::command]
pub async fn add_file(state: State<'_, AppState>, file: String) -> Result<(), ()> {
  let mut state = state.lock().await;
  log::debug!("Adding file: {}", file);
  state.schedule_upload(file);
  Ok(())
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> Result<String, ()> {
  let state = state.lock().await;
  
  let mut files = state.files.iter().map(|file| (file, damerau_levenshtein(&file.name, &query))).collect::<Vec<_>>();
  files.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

  Ok(serde_json::to_string(&files.iter().filter(|(_, distance)| *distance < 0.5).map(|(file, _)| file).collect::<Vec<_>>()).unwrap())
}

#[tauri::command]
pub async fn delete_file(state: State<'_, AppState>, file: u64) -> Result<(), ()> {
  println!("Deleting file: {}", file);
  Ok(())
}
