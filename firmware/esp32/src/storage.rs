//! Settings and recipes as JSON files on the LittleFS data partition.

use std::ffi::CString;
use std::fs;

use esp_idf_svc::sys::{esp, esp_vfs_littlefs_conf_t, esp_vfs_littlefs_register, EspError};
use pourcore::settings::Settings;
use serde_json::Value;

const BASE: &str = "/littlefs";
/// The Arduino partition name, kept so existing data is picked up.
const PARTITION: &str = "spiffs";
pub const SETTINGS_PATH: &str = "/littlefs/settings.json";
pub const RECIPES_PATH: &str = "/littlefs/recipes.json";

/// Mounts the data partition, formatting it if it holds no filesystem.
pub fn mount() -> Result<(), EspError> {
    // Leaked on purpose: the VFS keeps these pointers for the program's life.
    let base = CString::new(BASE).unwrap().into_raw();
    let label = CString::new(PARTITION).unwrap().into_raw();
    let mut conf = esp_vfs_littlefs_conf_t { base_path: base, partition_label: label, ..Default::default() };
    conf.set_format_if_mount_failed(1);
    esp!(unsafe { esp_vfs_littlefs_register(&conf) })
}

fn read_json(path: &str) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

pub fn load_settings() -> Settings {
    // First boot: keep defaults.
    read_json(SETTINGS_PATH).map(|v| Settings::from_storage_json(&v)).unwrap_or_default()
}

pub fn save_settings(s: &Settings) -> bool {
    fs::write(SETTINGS_PATH, s.to_storage_json().to_string()).is_ok()
}

pub fn read_recipes() -> Option<Value> {
    read_json(RECIPES_PATH).filter(Value::is_array)
}

/// Writes the bundled recipes if the file is missing or unreadable.
pub fn ensure_default_recipes(defaults: &str) {
    if read_recipes().is_none() {
        if let Err(e) = fs::write(RECIPES_PATH, defaults) {
            log::error!("could not write default recipes: {e}");
        }
    }
}

pub fn save_recipes(recipes: &Value) -> bool {
    recipes.is_array() && fs::write(RECIPES_PATH, recipes.to_string()).is_ok()
}
