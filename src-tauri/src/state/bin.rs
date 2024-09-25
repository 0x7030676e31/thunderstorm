use super::model::CURRENT_VERSION;
use crate::utils::path;

use std::{fs, io};

mod v1 {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    pub struct State {
        pub next_id: u32,
        pub channel_id: String,
        pub guild_id: String,
        pub token: String,
        pub do_encrypt: bool,
        pub do_checksum: bool,
        pub files: Vec<File>,
    }

    #[derive(Deserialize, Serialize)]
    pub struct File {
        pub id: u32,
        pub path: String,
        pub name: Option<String>,
        pub size: u64,
        pub download_ids: Vec<u64>,
        pub created_at: u64,
        pub updated_at: u64,
        pub crc32: u32,
        #[serde(with = "serde_bytes")]
        pub encryption_key: Option<[u8; 32]>,
    }
}

mod v2 {
    use crate::state::bin::v1;
    use crate::utils::download_path;

    use bincode::{deserialize, serialize, Result};
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    pub struct State {
        pub next_id: u32,
        pub channel_id: String,
        pub guild_id: String,
        pub token: String,
        pub do_encrypt: bool,
        pub do_checksum: bool,
        pub download_location: String,
        pub files: Vec<super::v1::File>,
    }

    pub fn from_v1(state: &[u8]) -> Result<Vec<u8>> {
        log::info!("upgrading state file from v1 to v2");
        let state = deserialize::<v1::State>(state)?;
        let state = State {
            next_id: state.next_id,
            channel_id: state.channel_id,
            guild_id: state.guild_id,
            token: state.token,
            do_encrypt: state.do_encrypt,
            do_checksum: state.do_checksum,
            download_location: download_path().to_string(),
            files: state.files,
        };

        serialize(&state)
    }
}

pub fn upgrade() -> io::Result<()> {
    let mut version = [0u8; 2];
    let path = format!("{}/state.bin", path());

    let file = fs::read(&path)?;

    if file.len() < 2 {
        log::error!("state file is too short");
        return Ok(());
    }

    version.copy_from_slice(&file[..2]);
    let version = u16::from_be_bytes(version);

    log::info!("state file version: {}", version);
    let mut state = file[2..].to_vec();

    if version == 1 {
        state = match v2::from_v1(&state) {
            Ok(state) => state,
            Err(err) => {
                log::error!("failed to upgrade state file: {}", err);
                return Ok(());
            }
        };
    }

    let state = [CURRENT_VERSION.to_be_bytes().to_vec(), state].concat();
    fs::write(&path, state)
}
