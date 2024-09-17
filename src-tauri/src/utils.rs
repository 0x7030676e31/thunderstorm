use std::{env, future::Future, path::Path, sync::OnceLock};

use tokio::task::{JoinError, JoinHandle};

pub fn path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| match env::consts::OS {
        "linux" => format!(
            "{}/.config/thunderstorm",
            env::var("HOME").expect("HOME not set")
        ),
        "windows" => format!(
            "{}/thunderstorm",
            env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set")
        ),
        "macos" => format!(
            "{}/Library/Application Support/thunderstorm",
            env::var("HOME").expect("HOME not set")
        ),
        _ => panic!("unsupported OS"),
    })
}

pub fn download_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| match env::consts::OS {
        "linux" => format!("{}/Downloads", env::var("HOME").expect("HOME not set")),
        "windows" => format!(
            "{}/Downloads",
            env::var("USERPROFILE").expect("USERPROFILE not set")
        ),
        "macos" => format!("{}/Downloads", env::var("HOME").expect("HOME not set")),
        _ => panic!("unsupported OS"),
    })
}

pub fn download_target(path: &str) -> String {
    let filename = match env::consts::OS {
        "linux" => format!(
            "{}/{}",
            download_path(),
            path.split('/').last().expect("failed to get filename")
        ),
        "windows" => format!(
            "{}\\{}",
            download_path(),
            path.split('\\').last().expect("failed to get filename")
        ),
        "macos" => format!(
            "{}/{}",
            download_path(),
            path.split('/').last().expect("failed to get filename")
        ),
        _ => panic!("unsupported OS"),
    };

    let (base, ext) = match filename.rsplit_once('.') {
        Some((base, ext)) => (base, ".".to_owned() + ext),
        None => (filename.as_str(), String::new()),
    };

    if Path::new(&filename).exists() {
        let mut i = 1;
        while Path::new(&format!("{} ({}){}", base, i, ext)).exists() {
            i += 1;
        }

        format!("{} ({}){}", base, i, ext)
    } else {
        filename
    }
}

pub trait Flatten<T, E1, E2>
where
    Self: Future<Output = Result<Result<T, E1>, E2>>,
    E1: Default,
{
    async fn flatten(self) -> Result<T, E1>;
}

impl<T, E> Flatten<T, E, JoinError> for JoinHandle<Result<T, E>>
where
    E: Default,
{
    async fn flatten(self) -> Result<T, E> {
        match self.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(E::default()),
        }
    }
}
