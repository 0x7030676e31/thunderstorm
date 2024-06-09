use crate::AppState;

use std::path::Path;
use std::sync::OnceLock;
use std::{env, fmt, fs, ptr};

use rand::Rng;
use tauri::AppHandle;
use serde::{Serialize, Deserialize};
use serde::de::{
  Deserializer,
  Error,
  IgnoredAny,
  MapAccess,
  Visitor,
};

fn path() -> &'static str {
  static PATH: OnceLock<String> = OnceLock::new();
  PATH.get_or_init(|| {
    match env::consts::OS {
      "linux" => format!("{}/.config/thunderstorm", env::var("HOME").expect("HOME not set")),
      "windows" => format!("{}/thunderstorm", env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set")),
      "macos" => format!("{}/Library/Application Support/thunderstorm", env::var("HOME").expect("HOME not set")),
      _ => panic!("unsupported OS"),
    }
  })
}

#[derive(Debug, Serialize)]
pub struct State {
  pub next_id: u32,
  pub channel_id: Option<String>,
  pub guild_id: Option<String>,
  pub token: Option<String>,
  pub files: Vec<File>,
  
  #[serde(with = "serde_bytes")]
  pub encryption_key: [u8; 64],
  #[serde(skip)]
  pub this: *const AppState,
  #[serde(skip)]
  pub app_handle: *const AppHandle,
  #[serde(skip)]
  pub queue: Vec<String>,
}

unsafe impl Send for State {}
unsafe impl Sync for State {}

impl<'de> Deserialize<'de> for State {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    const FIELDS: &[&str] = &["next_id", "channel_id", "guild_id", "token", "files", "encryption_key"];

    struct StateVisitor;

    impl<'de> Visitor<'de> for StateVisitor {
      type Value = State;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct State")
      }

      fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
      where
        V: MapAccess<'de>,
      {
        let mut next_id = None;
        let mut channel_id = None;
        let mut guild_id = None;
        let mut token = None;
        let mut files = None;
        let mut encryption_key = None;

        while let Some(key) = map.next_key()? {
          match key {
            "next_id" => {
              if next_id.is_some() {
                return Err(Error::duplicate_field("next_id"));
              }
              next_id = Some(map.next_value()?);
            }
            "channel_id" => {
              if channel_id.is_some() {
                return Err(Error::duplicate_field("channel_id"));
              }
              channel_id = Some(map.next_value()?);
            }
            "guild_id" => {
              if guild_id.is_some() {
                return Err(Error::duplicate_field("guild_id"));
              }
              guild_id = Some(map.next_value()?);
            }
            "token" => {
              if token.is_some() {
                return Err(Error::duplicate_field("token"));
              }
              token = Some(map.next_value()?);
            }
            "files" => {
              if files.is_some() {
                return Err(Error::duplicate_field("files"));
              }
              files = Some(map.next_value()?);
            }
            "encryption_key" => {
              if encryption_key.is_some() {
                return Err(Error::duplicate_field("encryption_key"));
              }

              let key = map.next_value::<&[u8]>()?;
              encryption_key = Some([0; 64]);

              encryption_key.as_mut().map(|k| {
                k.copy_from_slice(key);
              });
            }
            _ => {
              let _ = map.next_value::<IgnoredAny>()?;
            }
          }
        }

        let next_id = next_id.ok_or_else(|| Error::missing_field("next_id"))?;
        let channel_id = channel_id.ok_or_else(|| Error::missing_field("channel_id"))?;
        let guild_id = guild_id.ok_or_else(|| Error::missing_field("guild_id"))?;
        let token = token.ok_or_else(|| Error::missing_field("token"))?;
        let files = files.ok_or_else(|| Error::missing_field("files"))?;
        let encryption_key = encryption_key.ok_or_else(|| Error::missing_field("encryption_key"))?;

        Ok(State {
          next_id,
          channel_id,
          guild_id,
          token,
          files,
          encryption_key,
          this: ptr::null(),
          app_handle: ptr::null(),
          queue: Vec::new(),
        })
      }
    }

    deserializer.deserialize_struct("State", FIELDS, StateVisitor)
  }
}

impl Default for State {
  fn default() -> Self {
    let mut rng = rand::thread_rng();
    let mut key = [0; 64];
    rng.fill(&mut key[..]);

    Self {
      next_id: 1,
      channel_id: None,
      guild_id: None,
      token: None,
      files: Vec::new(),
      encryption_key: key,
      this: ptr::null(),
      app_handle: ptr::null(),
      queue: Vec::new(),
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct File {
  pub id: u32,
  pub name: String,
  pub size: u64,
  pub download_ids: Vec<String>,
  pub created_at: String,
  pub updated_at: String,
  pub crc32: u32,
}

impl State {
  pub fn new() -> Self {
    let app_data = path();
    if !Path::new(app_data).exists() {
      fs::create_dir_all(app_data).expect("failed to create app data directory");
    }

    let state_file = format!("{}/state.json", app_data);
    if !Path::new(&state_file).exists() {
      return Self::default();
    }

    let file = match fs::read(&state_file) {
      Ok(file) => file,
      Err(e) => {
        log::error!("failed to read state file, launching with default state: {}", e);
        return Self::default();
      }
    };

    match bincode::deserialize(&file) {
      Ok(state) => state,
      Err(e) => {
        log::error!("failed to deserialize state file, launching with default state: {}", e);
        Self::default()
      }
    }
  }

  pub fn write(&self) {
    let state_file = format!("{}/state.json", path());
    let state = match bincode::serialize(&self) {
      Ok(state) => state,
      Err(e) => {
        log::error!("failed to serialize state, not writing state file: {}", e);
        return;
      }
    };

    if let Err(e) = fs::write(&state_file, state) {
      log::error!("failed to write state file: {}", e);
    }
  }

  pub fn extend_queue(&mut self, files: Vec<String>) {
    self.queue.extend(files);
  }
}
