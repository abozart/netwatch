//! UI widgets extracted from app.rs. Each submodule owns one concern
//! (chart, titlebar, options menu, process table, action menu, resize grip,
//! hotkey modal) and attaches methods to `NetWatchApp` via split `impl`
//! blocks so call sites in `app.rs::update` stay byte-identical.

pub mod action_menu;
pub mod chart;
pub mod hotkey_modal;
pub mod options_menu;
pub mod resize_grip;
pub mod table;
pub mod titlebar;
