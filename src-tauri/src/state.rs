use crate::AppState;

use tauri::AppHandle;

#[derive(Default)]
pub struct State {
  pub this: Option<AppState>,
  pub app_handle: Option<AppHandle>,
}

impl State {
  // pub fn new() -> Self {
  //   State {
  //     this: None,
  //   }
  // }
} 