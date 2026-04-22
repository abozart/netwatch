use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::defaults;
use crate::features::FeatureToggle;
use crate::state::{AppState, SortBy, SortDir};
use crate::ui::chart::ChartStyle;

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
    #[serde(default = "default_true")]
    pub show_peak_avg: bool,
    #[serde(default = "default_true")]
    pub show_chart_axes: bool,
    #[serde(default = "default_true")]
    pub show_background: bool,
    #[serde(default)]
    pub hide_from_taskbar: bool,
    pub sort_by: SortBy,
    pub sort_dir: SortDir,
    #[serde(default)]
    pub chart_style: ChartStyle,
    /// feature_key (e.g. "click_through") → combo ("Ctrl+Alt+Shift+T")
    #[serde(default)]
    pub hotkeys: HashMap<String, String>,
    /// Inner size of the window at last exit. `None` falls back to defaults.
    #[serde(default)]
    pub window_size: Option<[f32; 2]>,
    /// Outer top-left position of the window at last exit (screen coords).
    #[serde(default)]
    pub window_pos: Option<[f32; 2]>,
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
            show_peak_avg: defaults::SHOW_PEAK_AVG,
            show_chart_axes: defaults::SHOW_CHART_AXES,
            show_background: defaults::SHOW_BACKGROUND,
            hide_from_taskbar: defaults::HIDE_FROM_TASKBAR,
            sort_by: defaults::SORT_BY,
            sort_dir: defaults::SORT_DIR,
            chart_style: defaults::CHART_STYLE,
            hotkeys,
            window_size: None,
            window_pos: None,
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
        state.chart_style = self.chart_style;
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
                FeatureToggle::ShowPeakAvg => self.show_peak_avg,
                FeatureToggle::ShowChartAxes => self.show_chart_axes,
                FeatureToggle::ShowBackground => self.show_background,
                FeatureToggle::HideFromTaskbar => self.hide_from_taskbar,
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
            show_peak_avg: FeatureToggle::ShowPeakAvg.get(state),
            show_chart_axes: FeatureToggle::ShowChartAxes.get(state),
            show_background: FeatureToggle::ShowBackground.get(state),
            hide_from_taskbar: FeatureToggle::HideFromTaskbar.get(state),
            sort_by: state.sort_by,
            sort_dir: state.sort_dir,
            chart_style: state.chart_style,
            hotkeys,
            // Populated only at exit via Settings::with_window_rect so per-frame
            // comparisons in app.rs don't spam-save while the user drags/resizes.
            window_size: None,
            window_pos: None,
        }
    }

    /// Stamp window geometry into a snapshot just before saving. Called from
    /// `on_exit` using whatever rect the app cached on its last update frame.
    pub fn with_window_rect(
        mut self,
        size: Option<[f32; 2]>,
        pos: Option<[f32; 2]>,
    ) -> Self {
        self.window_size = size;
        self.window_pos = pos;
        self
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
