use std::sync::OnceLock;
use std::{env, fs};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct File {
  pub name: String,
  pub size: u64,
  pub clusters: Vec<String>,
  pub modified: u64,
  pub created: u64,
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
  pub token: Option<String>,
  pub aes_key: Option<String>,
  pub storage_channel: Option<String>,
  pub files: Vec<File>,
}

fn path() -> &'static str {
  static PATH: OnceLock<String> = OnceLock::new();
  PATH.get_or_init(|| {
    match env::consts::OS {
      "linux" => format!("{}/.thunderstorm", env::var("HOME").unwrap()),
      "windows" => format!("{}\\Thunderstorm.json", env::var("APPDATA").unwrap()),
      _ => panic!("Unsupported OS"),
    }
  })
}

impl State {
  pub fn new() -> Self {
    if fs::metadata(path()).is_err() {
      return Self::default();
    }

    let data = match fs::read_to_string(path()) {
      Ok(data) => data,
      Err(e) => {
        log::error!("Error reading state from disk: {}", e);
        return Self::default();
      }
    };

    match serde_json::from_str(&data) {
      Ok(state) => state,
      Err(e) => {
        log::error!("Error parsing state from disk: {}", e);
        Self::default()
      }
    }
  }

  pub fn write(&self) {
    let data = serde_json::to_string(self).unwrap();
    match fs::write(path(), data) {
      Ok(_) => log::debug!("State written to disk"),
      Err(e) => log::error!("Error writing state to disk: {}", e),
    }
  }
}