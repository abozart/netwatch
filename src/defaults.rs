
//! Single source of truth for default values. Every hardcoded UI/UX default
//! lives here — `AppState::new`, `Settings::default`, `FeatureToggle`, the
//! viewport builder, the opacity-slider bounds, the show/hide resize targets,
//! and anything else that would otherwise drift across files.

use crate::state::{SortBy, SortDir};
use crate::ui::chart::ChartStyle;

// --- Toggles ---------------------------------------------------------------

pub const ALWAYS_ON_TOP: bool = true;
pub const SHOW_TITLE_BAR: bool = true;
pub const SHOW_PROCESSES: bool = true;
pub const CLICK_THROUGH: bool = false;
pub const MINIMIZE_TO_TRAY_ON_CLOSE: bool = false;
pub const PAUSE: bool = false;
pub const SHOW_PEAK_AVG: bool = true;
pub const SHOW_CHART_AXES: bool = true;
pub const SHOW_BACKGROUND: bool = true;
pub const HIDE_FROM_TASKBAR: bool = false;

// --- Appearance ------------------------------------------------------------

pub const OPACITY: f32 = 0.65;
pub const OPACITY_MIN: f32 = 0.15;
pub const OPACITY_MAX: f32 = 1.0;

// --- Window sizes ----------------------------------------------------------

pub const WINDOW_WIDTH: f32 = 560.0;
pub const WINDOW_HEIGHT_WITH_PROCESSES: f32 = 360.0;
pub const WINDOW_HEIGHT_NO_PROCESSES: f32 = 180.0;
pub const WINDOW_MIN_WIDTH: f32 = 320.0;
pub const WINDOW_MIN_HEIGHT: f32 = 140.0;

// --- Table / sort ----------------------------------------------------------

pub const SORT_BY: SortBy = SortBy::DownRate;
pub const SORT_DIR: SortDir = SortDir::Desc;

// --- Chart style -----------------------------------------------------------

/// Default chart style — the current thin-line rendering. Swappable at
/// runtime via the Options menu; other variants are defined in
/// [`crate::ui::chart::ChartStyle`].
pub const CHART_STYLE: ChartStyle = ChartStyle::Line;

// --- Chart -----------------------------------------------------------------

/// Fixed chart height when the process table is visible beneath it. When the
/// process table is hidden the chart grows to fill the remaining window
/// height instead of using this constant.
pub const CHART_HEIGHT_WITH_PROCESSES: f32 = 110.0;

/// Minimum chart height when the process table is hidden, so the chart never
/// collapses to an unreadable slice if the user drags the window very small.
pub const CHART_HEIGHT_MIN: f32 = 60.0;

// --- Menus -----------------------------------------------------------------

/// Max height of the Options menu popup before its contents start scrolling.
/// Keeps the menu reachable on small screens regardless of how many toggles
/// exist.
pub const OPTIONS_MENU_MAX_HEIGHT: f32 = 420.0;

// --- Icons -----------------------------------------------------------------

/// Icon size used for the window/taskbar. Windows scales down to 16/32 for
/// the taskbar and up for alt-tab from this source.
pub const WINDOW_ICON_SIZE: u32 = 64;
/// Icon size used for the system tray. Kept at 16 so our dynamic Up/Dn
/// meter renders pixel-sharp — a larger source buffer would just be scaled
/// down by Windows and blur the already-tiny glyphs.
pub const TRAY_ICON_SIZE: u32 = 16;

// --- Hotkeys ---------------------------------------------------------------

/// Fallback combo pre-bound for click-through on first launch, so the user
/// can always turn off click-through even if they never configure a hotkey.
pub const CLICK_THROUGH_DEFAULT_HOTKEY: &str = "Ctrl+Alt+Shift+T";

// --- Timing ----------------------------------------------------------------

/// How often (ms) egui should repaint without external input.
pub const UI_REPAINT_INTERVAL_MS: u64 = 250;
/// How often (seconds) to poll Win32_Service + firewall rules.
pub const SERVICE_REFRESH_INTERVAL_SECS: u64 = 8;
