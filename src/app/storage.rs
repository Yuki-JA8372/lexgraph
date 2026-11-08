//! `.tgf` binary files (magic, version, kind, payload) and JSON export.
use super::model::{Command, Document, KeyCombo, Library, SavedExpr, Settings, Shortcuts};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Version 2 added `Settings::png_size`. Other file kinds are unchanged from version 1.
pub const TGF_VERSION: u16 = 2;
const JSON_VERSION: u32 = 1;
const MAGIC: &[u8; 4] = b"TGF\0";
const HEADER_LEN: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Document = 1,
    Library = 2,
    Settings = 3,
    Session = 4,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("LexGraph のファイル (.tgf) ではありません")]
    NotTgf,
    #[error("このファイルは新しいバージョン ({0}) で保存されています")]
    UnsupportedVersion(u32),
    #[error("ファイルの種類が違います")]
    WrongKind,
    #[error("ファイルが壊れています: {0}")]
    Corrupt(String),
    #[error("JSON を読み書きできません: {0}")]
    Json(String),
    #[error("ファイルを読み書きできません: {0}")]
    Io(#[from] std::io::Error),
}

pub fn encode<T: Serialize>(kind: FileKind, value: &T) -> Result<Vec<u8>, StorageError> {
    let payload = postcard::to_stdvec(value).map_err(|e| StorageError::Corrupt(e.to_string()))?;
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&TGF_VERSION.to_le_bytes());
    out.push(kind as u8);
    out.extend_from_slice(&payload);
    Ok(out)
}

/// The settings payload of version 1 files.
#[derive(Deserialize)]
struct SettingsV1 {
    shortcuts: Shortcuts,
    transparent_export: bool,
}

/// Reads settings of any supported version. Version 1's default graph-copy key
/// moves to the new default, unless the user already gave that key another use.
pub fn decode_settings(bytes: &[u8]) -> Result<Settings, StorageError> {
    let is_v1 = bytes.len() >= HEADER_LEN && &bytes[..4] == MAGIC && bytes[4..6] == [1, 0];
    if !is_v1 {
        return decode(FileKind::Settings, bytes);
    }
    let old: SettingsV1 = decode(FileKind::Settings, bytes)?;
    let mut shortcuts = old.shortcuts;
    if shortcuts.lookup(&KeyCombo::ctrl_shift("c")) == Some(Command::CopyGraph) {
        // A refused rebind means the new key is taken, so the old one stays.
        let _ = shortcuts.rebind(Command::CopyGraph, KeyCombo::ctrl_shift("g"));
    }
    Ok(Settings {
        shortcuts,
        transparent_export: old.transparent_export,
        ..Settings::default()
    })
}

pub fn decode<T: DeserializeOwned>(kind: FileKind, bytes: &[u8]) -> Result<T, StorageError> {
    if bytes.len() < HEADER_LEN || &bytes[..4] != MAGIC {
        return Err(StorageError::NotTgf);
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version > TGF_VERSION {
        return Err(StorageError::UnsupportedVersion(u32::from(version)));
    }
    if bytes[6] != kind as u8 {
        return Err(StorageError::WrongKind);
    }
    postcard::from_bytes(&bytes[HEADER_LEN..]).map_err(|e| StorageError::Corrupt(e.to_string()))
}

/// Writes through a temporary file, so a crash cannot leave a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}

/// Reads and decodes a `.tgf` file.
pub fn read_tgf<T: DeserializeOwned>(kind: FileKind, path: &Path) -> Result<T, StorageError> {
    decode(kind, &std::fs::read(path)?)
}

/// Encodes and writes a `.tgf` file.
pub fn write_tgf<T: Serialize>(kind: FileKind, path: &Path, value: &T) -> Result<(), StorageError> {
    write_atomic(path, &encode(kind, value)?)
}

/// `%APPDATA%\LexGraph`, where the session, library and settings live.
pub fn app_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("LexGraph")
}

/// What is restored at the next launch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub document: Document,
    pub path: Option<String>,
    pub pane_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportBundle {
    pub version: u32,
    pub saved: Vec<SavedExpr>,
    pub settings: Settings,
}

pub fn export_json(library: &Library, settings: &Settings) -> Result<String, StorageError> {
    let bundle = ExportBundle {
        version: JSON_VERSION,
        saved: library.saved.clone(),
        settings: settings.clone(),
    };
    serde_json::to_string_pretty(&bundle).map_err(|e| StorageError::Json(e.to_string()))
}

pub fn import_json(text: &str) -> Result<ExportBundle, StorageError> {
    let json = |e: serde_json::Error| StorageError::Json(e.to_string());
    let value: serde_json::Value = serde_json::from_str(text).map_err(json)?;
    let version = value.get("version").and_then(serde_json::Value::as_u64);
    if let Some(version) = version.filter(|v| *v > u64::from(JSON_VERSION)) {
        return Err(StorageError::UnsupportedVersion(version as u32));
    }
    serde_json::from_value(value).map_err(json)
}
