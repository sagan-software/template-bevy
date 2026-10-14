//! Versioned settings and progress persistence.
use bevy::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;

/// Version 2 save envelope. Games can extend Progress in a future schema.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveData {
    /// Current schema version. Future versions are rejected without overwriting.
    pub version: u32,
    /// Master audio settings.
    pub settings: crate::AudioSettings,
    /// Shared run counters.
    pub progress: Progress,
}

/// Minimal progress shared across prototypes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// Completed runs.
    pub runs: u32,
    /// Highest score.
    pub best_score: u32,
}

/// Platform save location. The identifier also namespaces browser localStorage.
#[derive(Resource, Debug, Clone)]
pub struct SaveStore {
    identifier: String,
    #[cfg(not(target_arch = "wasm32"))]
    override_directory: Option<std::path::PathBuf>,
}

/// Persistence or schema failure. Callers decide how to show recovery.
#[derive(Debug)]
pub struct SaveError(pub String);
impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SaveError {}
impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: 2,
            settings: default(),
            progress: default(),
        }
    }
}
impl SaveData {
    /// Serialize only current-version data with finite, bounded master volume.
    pub fn encode(&self) -> Result<String, SaveError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|e| SaveError(e.to_string()))
    }
    /// Read version 2, or migrate version 1 `{version, muted, best_score}`.
    pub fn decode(json: &str) -> Result<Self, SaveError> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| SaveError(e.to_string()))?;
        let save = match value.get("version").and_then(|v| v.as_u64()) {
            Some(2) => serde_json::from_value(value).map_err(|e| SaveError(e.to_string()))?,
            Some(1) => {
                #[derive(Deserialize)]
                struct Legacy {
                    muted: bool,
                    best_score: u32,
                }
                let old: Legacy =
                    serde_json::from_value(value).map_err(|e| SaveError(e.to_string()))?;
                Self {
                    settings: crate::AudioSettings {
                        muted: old.muted,
                        ..default()
                    },
                    progress: Progress {
                        best_score: old.best_score,
                        ..default()
                    },
                    ..default()
                }
            }
            _ => return Err(SaveError("Unsupported or missing save version".into())),
        };
        save.validate()?;
        Ok(save)
    }
    fn validate(&self) -> Result<(), SaveError> {
        if self.version != 2
            || !self.settings.master.is_finite()
            || !(0.0..=1.0).contains(&self.settings.master)
        {
            return Err(SaveError("Invalid save version or master volume".into()));
        }
        Ok(())
    }
}
impl SaveStore {
    /// Use a stable, ASCII application identifier containing letters, digits, dots or hyphens.
    pub fn new(identifier: &str) -> Result<Self, SaveError> {
        if identifier.is_empty()
            || !identifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        {
            return Err(SaveError("Invalid save identifier".into()));
        }
        Ok(Self {
            identifier: identifier.into(),
            #[cfg(not(target_arch = "wasm32"))]
            override_directory: None,
        })
    }
    /// Override the native application directory for tests or a platform host.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_directory(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.override_directory = Some(path.into());
        self
    }
    /// Missing saves start at defaults. Corrupt or unsupported saves return an error.
    pub fn load(&self) -> Result<SaveData, SaveError> {
        self.read()?
            .map_or_else(|| Ok(SaveData::default()), |json| SaveData::decode(&json))
    }
    /// Persist through an atomic file replacement or localStorage setItem.
    pub fn store(&self, save: &SaveData) -> Result<(), SaveError> {
        self.write(&save.encode()?)
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn directory(&self) -> Result<std::path::PathBuf, SaveError> {
        if let Some(path) = &self.override_directory {
            return Ok(path.clone());
        }
        #[cfg(target_os = "android")]
        if let Some(app) = bevy::android::ANDROID_APP.get() {
            if let Some(path) = app.internal_data_path() {
                return Ok(path.join(&self.identifier));
            }
        }
        directories::ProjectDirs::from("software", "sagan", &self.identifier)
            .map(|d| d.data_local_dir().to_path_buf())
            .ok_or_else(|| SaveError("Application data directory unavailable".into()))
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn read(&self) -> Result<Option<String>, SaveError> {
        match std::fs::read_to_string(self.directory()?.join("save.json")) {
            Ok(json) => Ok(Some(json)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SaveError(e.to_string())),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn write(&self, json: &str) -> Result<(), SaveError> {
        let dir = self.directory()?;
        std::fs::create_dir_all(&dir).map_err(|e| SaveError(e.to_string()))?;
        let temp = dir.join("save.tmp");
        std::fs::write(&temp, json).map_err(|e| SaveError(e.to_string()))?;
        std::fs::rename(temp, dir.join("save.json")).map_err(|e| SaveError(e.to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    fn storage(&self) -> Result<web_sys::Storage, SaveError> {
        web_sys::window()
            .ok_or_else(|| SaveError("Browser window unavailable".into()))?
            .local_storage()
            .map_err(|e| SaveError(format!("localStorage denied: {e:?}")))?
            .ok_or_else(|| SaveError("localStorage unavailable".into()))
    }
    #[cfg(target_arch = "wasm32")]
    fn read(&self) -> Result<Option<String>, SaveError> {
        self.storage()?
            .get_item(&self.identifier)
            .map_err(|e| SaveError(format!("Save read failed: {e:?}")))
    }
    #[cfg(target_arch = "wasm32")]
    fn write(&self, json: &str) -> Result<(), SaveError> {
        self.storage()?
            .set_item(&self.identifier, json)
            .map_err(|e| SaveError(format!("Save write failed: {e:?}")))
    }
}
