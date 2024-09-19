use std::fmt::Display;
use std::io;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

#[derive(Debug)]
pub enum UploadError {
    Io(io::Error),
    Reqwest(reqwest::Error),
    Unauthorized, // 401
    Forbidden,    // 403
    NotFound,     // 404
    Unknown((u16, String)),
    JoinError,
}

impl From<reqwest::Error> for UploadError {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
    }
}

impl Serialize for UploadError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("UploadError", 2)?;
        match *self {
            UploadError::Io(ref err) => {
                state.serialize_field("type", "Io")?;
                state.serialize_field("message", &err.to_string())?;
            }
            UploadError::Reqwest(ref err) => 'reqwest: {
                state.serialize_field("type", "Reqwest")?;
                if err.is_connect() {
                    state.serialize_field("message", "Could not open a connection. Check your internet connection and try again.")?;
                    break 'reqwest;
                }

                if err.is_timeout() {
                    state.serialize_field(
                        "message",
                        "Connection timed out. Check your internet connection and try again.",
                    )?;
                    break 'reqwest;
                }

                state.serialize_field("message", &err.to_string())?;
            }
            UploadError::Unauthorized => {
                state.serialize_field("type", "Unauthorized")?;
                state.serialize_field("message", "")?;
            }
            UploadError::Forbidden => {
                state.serialize_field("type", "Forbidden")?;
                state.serialize_field("message", "")?;
            }
            UploadError::NotFound => {
                state.serialize_field("type", "NotFound")?;
                state.serialize_field("message", "")?;
            }
            UploadError::Unknown((code, ref message)) => {
                state.serialize_field("type", "Unknown")?;
                state.serialize_field("message", &format!("{} - {}", code, message))?;
            }
            UploadError::JoinError => {
                state.serialize_field("type", "JoinError")?;
                state.serialize_field("message", "")?;
            }
        }
        state.end()
    }
}

impl Default for UploadError {
    fn default() -> Self {
        Self::JoinError
    }
}

impl Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "Io: {}", err),
            Self::Reqwest(err) => write!(f, "Reqwest: {}", err),
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::Forbidden => write!(f, "Forbidden"),
            Self::NotFound => write!(f, "Not Found"),
            Self::Unknown((status, message)) => write!(f, "Unknown: {} - {}", status, message),
            Self::JoinError => write!(f, "Join Error"),
        }
    }
}

#[derive(Debug)]
pub enum DownloadError {
    Io(io::Error),
    Reqwest(reqwest::Error),
    Unauthorized, // 401
    Forbidden,    // 403
    NotFound,     // 404
    Unknown((u16, String)),
    NotFoundLocal,
    JoinError,
    ChecksumMismatch(u32, u32),
    EncryptionError(String),
}

impl From<reqwest::Error> for DownloadError {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
    }
}

impl From<aes_gcm::Error> for DownloadError {
    fn from(value: aes_gcm::Error) -> Self {
        Self::EncryptionError(value.to_string())
    }
}

impl From<io::Error> for DownloadError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl Serialize for DownloadError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DownloadError", 2)?;
        match *self {
            DownloadError::Io(ref err) => {
                state.serialize_field("type", "Io")?;
                state.serialize_field("message", &err.to_string())?;
            }
            DownloadError::Reqwest(ref err) => 'reqwest: {
                state.serialize_field("type", "Reqwest")?;
                if err.is_connect() {
                    state.serialize_field("message", "Could not open a connection. Check your internet connection and try again.")?;
                    break 'reqwest;
                }

                if err.is_timeout() {
                    state.serialize_field(
                        "message",
                        "Connection timed out. Check your internet connection and try again.",
                    )?;
                    break 'reqwest;
                }

                state.serialize_field("message", &err.to_string())?;
            }
            DownloadError::Unauthorized => {
                state.serialize_field("type", "Unauthorized")?;
                state.serialize_field("message", "")?;
            }
            DownloadError::Forbidden => {
                state.serialize_field("type", "Forbidden")?;
                state.serialize_field("message", "")?;
            }
            DownloadError::NotFound => {
                state.serialize_field("type", "NotFound")?;
                state.serialize_field("message", "")?;
            }
            DownloadError::Unknown((code, ref message)) => {
                state.serialize_field("type", "")?;
                state.serialize_field("message", &format!("{} {}", code, message))?;
            }
            DownloadError::NotFoundLocal => {
                state.serialize_field("type", "NotFoundLocal")?;
                state.serialize_field("message", "")?;
            }
            DownloadError::JoinError => {
                state.serialize_field("type", "JoinError")?;
                state.serialize_field("message", "")?;
            }
            DownloadError::ChecksumMismatch(expected, actual) => {
                state.serialize_field("type", "ChecksumMismatch")?;
                state.serialize_field(
                    "message",
                    &format!("Expected: {}, Actual: {}", expected, actual),
                )?;
            }
            DownloadError::EncryptionError(ref message) => {
                state.serialize_field("type", "EncryptionError")?;
                state.serialize_field("message", message)?;
            }
        }
        state.end()
    }
}

impl Default for DownloadError {
    fn default() -> Self {
        Self::JoinError
    }
}

impl Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "Io: {}", err),
            Self::Reqwest(err) => write!(f, "Reqwest: {}", err),
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::Forbidden => write!(f, "Forbidden"),
            Self::NotFound => write!(f, "Not Found"),
            Self::Unknown((status, message)) => write!(f, "Unknown: {} - {}", status, message),
            Self::NotFoundLocal => write!(f, "Not Found Locally"),
            Self::JoinError => write!(f, "Join Error"),
            Self::ChecksumMismatch(expected, actual) => {
                write!(
                    f,
                    "Checksum Mismatch: Expected: {}, Actual: {}",
                    expected, actual
                )
            }
            Self::EncryptionError(err) => write!(f, "Encryption Error: {}", err),
        }
    }
}
