use crate::defaults;
use crate::state::AppState;

/// Single source of truth for every user-toggleable boolean. Everything else
/// (Options menu, tray menu, hotkey manager, settings serializer) iterates
/// [`FeatureToggle::ALL`] and uses these methods; adding a new toggle means
/// touching only this file.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FeatureToggle {
    AlwaysOnTop,
    ShowTitleBar,
    ShowProcesses,
    Pause,
    ClickThrough,
    MinimizeToTrayOnClose,
    ShowPeakAvg,
    ShowChartAxes,
    ShowBackground,
    HideFromTaskbar,
}

impl FeatureToggle {
    pub const ALL: &'static [Self] = &[
        Self::AlwaysOnTop,
        Self::ShowTitleBar,
        Self::ShowProcesses,
        Self::Pause,
        Self::ClickThrough,
        Self::MinimizeToTrayOnClose,
        Self::ShowPeakAvg,
        Self::ShowChartAxes,
        Self::ShowBackground,
        Self::HideFromTaskbar,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::AlwaysOnTop => "Always on top",
            Self::ShowTitleBar => "Show title bar",
            Self::ShowProcesses => "Show processes",
            Self::Pause => "Pause",
            Self::ClickThrough => "Click-through",
            Self::MinimizeToTrayOnClose => "Minimize to tray on close",
            Self::ShowPeakAvg => "Show peak/avg lines",
            Self::ShowChartAxes => "Show chart axes & grid",
            Self::ShowBackground => "Show background",
            Self::HideFromTaskbar => "Hide from taskbar",
        }
    }

    /// Stable key used in `settings.json`'s `hotkeys` map.
    pub fn settings_key(&self) -> &'static str {
        match self {
            Self::AlwaysOnTop => "always_on_top",
            Self::ShowTitleBar => "show_title_bar",
            Self::ShowProcesses => "show_processes",
            Self::Pause => "pause",
            Self::ClickThrough => "click_through",
            Self::MinimizeToTrayOnClose => "minimize_to_tray_on_close",
            Self::ShowPeakAvg => "show_peak_avg",
            Self::ShowChartAxes => "show_chart_axes",
            Self::ShowBackground => "show_background",
            Self::HideFromTaskbar => "hide_from_taskbar",
        }
    }

    /// Default global hotkey string understood by `global_hotkey::HotKey::from_str`.
    /// `ClickThrough` MUST have a default so the user can always turn click-through
    /// off even if their window is currently ignoring all input.
    pub fn default_hotkey(&self) -> Option<&'static str> {
        match self {
            Self::ClickThrough => Some(defaults::CLICK_THROUGH_DEFAULT_HOTKEY),
            _ => None,
        }
    }

    pub fn get(&self, st: &AppState) -> bool {
        match self {
            Self::AlwaysOnTop => st.always_on_top,
            Self::ShowTitleBar => st.show_title_bar,
            Self::ShowProcesses => st.show_processes,
            Self::Pause => st.paused,
            Self::ClickThrough => st.click_through,
            Self::MinimizeToTrayOnClose => st.minimize_to_tray_on_close,
            Self::ShowPeakAvg => st.show_peak_avg,
            Self::ShowChartAxes => st.show_chart_axes,
            Self::ShowBackground => st.show_background,
            Self::HideFromTaskbar => st.hide_from_taskbar,
        }
    }

    /// Pure state write — no side effects. Safe to call without an egui context
    /// or HWND. Used by settings load, tests, and as the first half of `set()`.
    pub fn write_state(&self, st: &mut AppState, value: bool) {
        match self {
            Self::AlwaysOnTop => st.always_on_top = value,
            Self::ShowTitleBar => st.show_title_bar = value,
            Self::ShowProcesses => st.show_processes = value,
            Self::Pause => st.paused = value,
            Self::ClickThrough => st.click_through = value,
            Self::MinimizeToTrayOnClose => st.minimize_to_tray_on_close = value,
            Self::ShowPeakAvg => st.show_peak_avg = value,
            Self::ShowChartAxes => st.show_chart_axes = value,
            Self::ShowBackground => st.show_background = value,
            Self::HideFromTaskbar => st.hide_from_taskbar = value,
        }
    }

    /// Fire the OS / viewport side effects for a toggle change. Called after
    /// `write_state` when we actually have a live ctx + hwnd (i.e. from the UI
    /// and event handlers, not from startup settings-load).
    pub fn apply_side_effects(
        &self,
        value: bool,
        ctx: &eframe::egui::Context,
        hwnd: Option<isize>,
    ) {
        use eframe::egui;
        match self {
            Self::AlwaysOnTop => {
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if value {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                }));
            }
            Self::ShowTitleBar => {
                // Decorations are tied to the native OS chrome, but we keep
                // `Resizable(true)` unconditionally so the chromeless window
                // can still be resized via the custom grip painted in
                // NetWatchApp::draw_resize_grip.
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(value));
                ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
            }
            Self::ShowProcesses => {
                let w = ctx.input(|i| {
                    i.viewport()
                        .inner_rect
                        .map(|r| r.width())
                        .unwrap_or(defaults::WINDOW_WIDTH)
                });
                let h = if value {
                    defaults::WINDOW_HEIGHT_WITH_PROCESSES
                } else {
                    defaults::WINDOW_HEIGHT_NO_PROCESSES
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
            }
            Self::Pause
            | Self::MinimizeToTrayOnClose
            | Self::ShowPeakAvg
            | Self::ShowChartAxes
            | Self::ShowBackground => {}
            Self::HideFromTaskbar => {
                #[cfg(windows)]
                if let Some(hwnd) = hwnd {
                    crate::click_through::set_toolwindow(hwnd, value);
                }
                let _ = hwnd;
            }
            Self::ClickThrough => {
                #[cfg(windows)]
                if let Some(hwnd) = hwnd {
                    crate::click_through::set(hwnd, value);
                }
                let _ = hwnd;
            }
        }
    }

    /// Combined write + side effects. This is what the Options menu / tray menu /
    /// hotkey handler all call.
    pub fn set(
        &self,
        st: &mut AppState,
        value: bool,
        ctx: &eframe::egui::Context,
        hwnd: Option<isize>,
    ) {
        self.write_state(st, value);
        self.apply_side_effects(value, ctx, hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_toggle_setter_roundtrip() {
        for &feat in FeatureToggle::ALL {
            let mut st = AppState::new();
            feat.write_state(&mut st, true);
            assert!(
                feat.get(&st),
                "write_state(true)/get mismatch for {feat:?}"
            );
            feat.write_state(&mut st, false);
            assert!(
                !feat.get(&st),
                "write_state(false)/get mismatch for {feat:?}"
            );
        }
    }

    #[test]
    fn feature_toggle_settings_keys_unique() {
        let mut seen = std::collections::HashSet::new();
        for feat in FeatureToggle::ALL {
            assert!(
                seen.insert(feat.settings_key()),
                "duplicate settings_key for {feat:?}"
            );
        }
    }
}
