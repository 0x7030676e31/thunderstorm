use crate::AppState;

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

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

#[tauri::command]
pub async fn download_files(_state: State<'_, AppState>, files: Vec<u32>) -> Result<(), ()> {
    log::debug!("Downloading files: {:?}", files);
    Ok(())
}

#[tauri::command]
pub async fn delete_files(state: State<'_, AppState>, files: Vec<u32>) -> Result<(), ()> {
    let mut state = state.write().await;
    log::debug!("Deleting {} files", files.len());

    state.files.retain(|file| !files.contains(&file.id));
    state.write();

    Ok(())
}

#[derive(Deserialize)]
pub struct PartialSettings {
    token: Option<String>,
    channel: Option<String>,
    guild: Option<String>,
}

#[tauri::command]
pub async fn set_settings(state: State<'_, AppState>, settings: PartialSettings) -> Result<(), ()> {
    let mut state = state.write().await;
    let mut earse_data = false;

    if let Some(token) = settings.token {
        state.token = token;
    }

    if let Some(channel) = settings.channel {
        state.channel_id = channel;
        earse_data = true;
    }

    if let Some(guild) = settings.guild {
        state.guild_id = guild;
        earse_data = true;
    }

    if earse_data {
        let handle = unsafe { state.rt.app_handle.as_ref().unwrap() };
        handle
            .emit_all("erase_files", ())
            .expect("failed to emit eraseData");

        log::info!("Changed sensitive settings, erasing data");
        state.files.clear();
    }

    state.write();
    Ok(())
}

#[tauri::command]
pub async fn cancel(state: State<'_, AppState>) -> Result<(), ()> {
    let mut state = state.write().await;
    log::debug!("Cancelling current action");
    state.cancel().await;
    Ok(())
}
