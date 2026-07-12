use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub discord_rpc: bool,
    pub ad_block: bool,
    pub close_to_tray: bool,
    pub launch_at_startup: bool,
    pub start_minimized: bool,
    pub zoom: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            discord_rpc: true,
            ad_block: true,
            close_to_tray: false,
            launch_at_startup: false,
            start_minimized: false,
            zoom: 1.0,
        }
    }
}

pub type SharedSettings = Arc<Mutex<Settings>>;

pub fn load() -> SharedSettings {
    let settings = settings_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();

    Arc::new(Mutex::new(settings))
}

pub fn snapshot(settings: &SharedSettings) -> Settings {
    settings
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default()
}

pub fn update(settings: &SharedSettings, change: impl FnOnce(&mut Settings)) {
    let Ok(mut value) = settings.lock() else {
        return;
    };

    change(&mut value);

    let Some(path) = settings_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    let Ok(json) = serde_json::to_string_pretty(&*value) else {
        return;
    };

    let _ = fs::create_dir_all(parent);
    let _ = fs::write(path, json);
}

fn settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|dir| dir.join("app.ytmusic.desktop").join("settings.json"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from).map(|dir| {
            dir.join(".config")
                .join("app.ytmusic.desktop")
                .join("settings.json")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_use_defaults() {
        let settings: Settings =
            serde_json::from_str(r#"{"discord_rpc":false}"#).expect("settings");

        assert!(!settings.discord_rpc);
        assert!(settings.ad_block);
        assert_eq!(settings.zoom, 1.0);
    }
}
