use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::defaults;
use crate::features::FeatureToggle;
use crate::state::{AppState, SortBy, SortDir};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub opacity: f32,
    pub always_on_top: bool,
    pub show_title_bar: bool,
    #[serde(default = "default_true")]
    pub show_processes: bool,
    #[serde(default)]
    pub click_through: bool,
    #[serde(default)]
    pub minimize_to_tray_on_close: bool,
    pub sort_by: SortBy,
    pub sort_dir: SortDir,
    /// feature_key (e.g. "click_through") → combo ("Ctrl+Alt+Shift+T")
    #[serde(default)]
    pub hotkeys: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        let mut hotkeys = HashMap::new();
        for feat in FeatureToggle::ALL {
            if let Some(combo) = feat.default_hotkey() {
                hotkeys.insert(feat.settings_key().to_string(), combo.to_string());
            }
        }
        Self {
            opacity: defaults::OPACITY,
            always_on_top: defaults::ALWAYS_ON_TOP,
            show_title_bar: defaults::SHOW_TITLE_BAR,
            show_processes: defaults::SHOW_PROCESSES,
            click_through: defaults::CLICK_THROUGH,
            minimize_to_tray_on_close: defaults::MINIMIZE_TO_TRAY_ON_CLOSE,
            sort_by: defaults::SORT_BY,
            sort_dir: defaults::SORT_DIR,
            hotkeys,
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("netwatch").join("settings.json"))
}

impl Settings {
    pub fn load() -> Self {
        let Some(path) = settings_path() else {
            return Settings::default();
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return Settings::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_else(|_| Settings::default())
    }

    pub fn save(&self) {
        let Some(path) = settings_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    pub fn apply_to(&self, state: &mut AppState) {
        state.opacity = self.opacity;
        state.sort_by = self.sort_by;
        state.sort_dir = self.sort_dir;
        // Apply every FeatureToggle via the single source of truth. Adding a new
        // feature to the enum means its write path here is automatic.
        for &feat in FeatureToggle::ALL {
            let val = match feat {
                FeatureToggle::AlwaysOnTop => self.always_on_top,
                FeatureToggle::ShowTitleBar => self.show_title_bar,
                FeatureToggle::ShowProcesses => self.show_processes,
                FeatureToggle::Pause => false, // pause never persists
                FeatureToggle::ClickThrough => self.click_through,
                FeatureToggle::MinimizeToTrayOnClose => self.minimize_to_tray_on_close,
            };
            feat.write_state(state, val);
        }
    }

    pub fn capture_from(state: &AppState) -> Self {
        let mut hotkeys = HashMap::new();
        // Preserve any existing bindings already registered at runtime by
        // reading straight from state.recording_hotkey / hotkey map via caller;
        // for now we just snapshot defaults and let app.rs fill from the live
        // HotkeyRegistry (see Settings::with_hotkeys).
        for feat in FeatureToggle::ALL {
            if let Some(combo) = feat.default_hotkey() {
                hotkeys.insert(feat.settings_key().to_string(), combo.to_string());
            }
        }
        Self {
            opacity: state.opacity,
            always_on_top: FeatureToggle::AlwaysOnTop.get(state),
            show_title_bar: FeatureToggle::ShowTitleBar.get(state),
            show_processes: FeatureToggle::ShowProcesses.get(state),
            click_through: FeatureToggle::ClickThrough.get(state),
            minimize_to_tray_on_close: FeatureToggle::MinimizeToTrayOnClose.get(state),
            sort_by: state.sort_by,
            sort_dir: state.sort_dir,
            hotkeys,
        }
    }

    /// Replace the `hotkeys` map with bindings currently held by the live
    /// hotkey registry. Call from `capture_from` in app.rs so saves reflect
    /// runtime rebinds.
    pub fn with_hotkeys(mut self, live: HashMap<String, String>) -> Self {
        self.hotkeys = live;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_identity() {
        let orig = Settings::default();
        let json = serde_json::to_string(&orig).unwrap();
        let parsed: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, parsed, "settings round-trip drifted");
    }

    #[test]
    fn settings_defaults_include_click_through_hotkey() {
        // Regression: ClickThrough must always have a default hotkey so the
        // user can escape a stuck click-through state.
        let s = Settings::default();
        assert!(
            s.hotkeys.contains_key("click_through"),
            "default settings missing click_through hotkey"
        );
    }
}
