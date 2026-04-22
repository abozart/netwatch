use eframe::egui;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

use crate::features::FeatureToggle;
use crate::hotkeys::HotkeyRegistry;
use crate::state::AppState;
use crate::tray::Tray;

/// A right-click menu frozen at the moment of the click. Its fields never change
/// while the menu is open, so reordering the table underneath can't affect which
/// process the menu targets.
#[derive(Clone)]
pub struct OpenMenu {
    pub pid: u32,
    pub name: String,
    pub pos: egui::Pos2,
    pub exe: Option<PathBuf>,
    pub services: Vec<String>,
    pub is_blocked: bool,
    /// Scheduled tasks whose first action targets this exe. Filled from a
    /// background thread because the PowerShell lookup takes 0.5–3 s; the
    /// menu opens instantly with an empty vec and the task buttons appear
    /// once the lookup completes. Shared via Arc<Mutex> so the worker can
    /// write without coordinating with the UI thread.
    #[cfg(windows)]
    pub tasks: Arc<parking_lot::Mutex<Vec<crate::tasks::TaskInfo>>>,
}

/// Top-level eframe application. Holds the shared `AppState`, tray handle,
/// hotkey registry, and window-level bookkeeping. Every user-visible widget
/// is extracted into `src/ui/*.rs` (chart, titlebar, options menu, process
/// table, action menu, resize grip, hotkey modal) and attaches methods to
/// this struct via its own `impl NetWatchApp` block, so the orchestration
/// here stays small.
pub struct NetWatchApp {
    pub(crate) state: Arc<RwLock<AppState>>,
    pub(crate) open_menu: Option<OpenMenu>,
    pub(crate) last_saved: crate::settings::Settings,
    pub(crate) tray: Option<Tray>,
    pub(crate) hotkeys: Option<HotkeyRegistry>,
    pub(crate) hwnd: Option<isize>,
    /// Last tooltip string pushed to the tray icon. Cached so we only issue
    /// the Win32 NIM_MODIFY call when the visible rate has actually changed.
    pub(crate) last_tray_tooltip: String,
    /// Last `ClickThrough` value written to the window style. `None` until
    /// the first write. Caching lets `update()` skip the `SetWindowLongW`
    /// call when nothing changed — the call is nominally idempotent on
    /// current Windows but still triggers a style-change message pump, so
    /// firing it every frame (~60×/sec) is wasteful.
    pub(crate) last_click_through: Option<bool>,
}

impl NetWatchApp {
    pub fn new(cc: &eframe::CreationContext<'_>, state: Arc<RwLock<AppState>>) -> Self {
        let last_saved = crate::settings::Settings::capture_from(&state.read());

        // Keep the update loop alive even when the window is hidden to tray,
        // so tray events (Show/Hide, Quit, Toggle click-through) and global
        // hotkeys still get drained. Cheap ~5 Hz wakeup.
        let ctx_ticker = cc.egui_ctx.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
            ctx_ticker.request_repaint();
        });

        let tray = match Tray::new() {
            Ok(t) => {
                t.spawn_event_thread(state.clone(), cc.egui_ctx.clone());
                Some(t)
            }
            Err(e) => {
                state.write().set_status(false, format!("Tray init failed: {e}"));
                None
            }
        };

        // Build hotkey registry and register whatever was saved in settings.
        let mut hotkeys = HotkeyRegistry::new().ok();
        if let Some(reg) = hotkeys.as_mut() {
            for feat in FeatureToggle::ALL {
                let key = feat.settings_key().to_string();
                let combo = last_saved
                    .hotkeys
                    .get(&key)
                    .cloned()
                    .or_else(|| feat.default_hotkey().map(String::from));
                if let Some(combo) = combo {
                    if let Err(e) = reg.bind(*feat, &combo) {
                        state.write().set_status(
                            false,
                            format!(
                                "Hotkey {combo} for {} unavailable — another app likely owns it. \
                                 Open Options and pick a different combo.",
                                feat.label()
                            ),
                        );
                        let _ = e; // detail in stderr via set_status mirror
                    }
                }
            }
        }

        Self {
            state,
            open_menu: None,
            last_saved,
            tray,
            hotkeys,
            hwnd: None,
            last_tray_tooltip: String::new(),
            last_click_through: None,
        }
    }
}

impl eframe::App for NetWatchApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Fully transparent framebuffer — Ui::set_opacity inside update() controls
        // the visual alpha of all rendered content uniformly (window-wide opacity).
        [0.0, 0.0, 0.0, 0.0]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let (size, pos) = {
            let s = self.state.read();
            (s.last_window_size, s.last_window_pos)
        };
        let snapshot = self.snapshot_settings().with_window_rect(size, pos);
        snapshot.save();
        #[cfg(windows)]
        {
            crate::etw::shutdown();
            crate::services::shutdown();
        }
        // Background threads (ETW capture, tray, global-hotkey) can hold the
        // process alive after eframe's event loop ends. Force-terminate so
        // `netwatch.exe` actually exits on any user's machine — no manual
        // Task Manager rescue needed.
        std::process::exit(0);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(
            crate::defaults::UI_REPAINT_INTERVAL_MS,
        ));

        // Snapshot current window geometry into AppState every frame so both
        // on_exit *and* the tray Quit handler (which runs on its own thread)
        // can persist the last-known rect. Zero-size rects aren't useful.
        let (size, pos) = ctx.input(|i| {
            let vp = i.viewport();
            let size = vp.inner_rect.and_then(|r| {
                (r.width() > 0.0 && r.height() > 0.0).then(|| [r.width(), r.height()])
            });
            let pos = vp.outer_rect.map(|r| [r.min.x, r.min.y]);
            (size, pos)
        });
        if size.is_some() || pos.is_some() {
            let mut st = self.state.write();
            if let Some(s) = size {
                st.last_window_size = Some(s);
            }
            if let Some(p) = pos {
                st.last_window_pos = Some(p);
            }
        }

        // Resolve HWND on first frame so later toggles can use it. On the
        // transition from None → Some, apply one-shot OS-level toggles that
        // Settings::apply_to couldn't touch (it only runs write_state). The
        // click-through flag gets its own per-frame enforcement below;
        // toolwindow/taskbar visibility we only set once to avoid the
        // hide+show flicker it requires.
        #[cfg(windows)]
        if self.hwnd.is_none() {
            self.hwnd = crate::win32::find_netwatch_hwnd();
            if let Some(hwnd) = self.hwnd {
                let hide_tb = FeatureToggle::HideFromTaskbar.get(&self.state.read());
                if hide_tb {
                    crate::click_through::set_toolwindow(hwnd, true);
                }
            }
        }

        // Keep the click-through WS_EX_TRANSPARENT flag in sync with state.
        // Settings::apply_to runs `write_state` only, so boot with
        // click-through pre-enabled never flipped the Win32 flag without
        // this — and a one-shot apply on first frame turned out to race
        // with eframe/winit's own style writes. Skipping when the cached
        // value matches keeps us off the per-frame SetWindowLongW path
        // while still reapplying on any real change (including the initial
        // transition from `None`).
        #[cfg(windows)]
        if let Some(hwnd) = self.hwnd {
            let on = FeatureToggle::ClickThrough.get(&self.state.read());
            if self.last_click_through != Some(on) {
                crate::click_through::set(hwnd, on);
                self.last_click_through = Some(on);
            }
        }

        // Push live Up/Dn into the tray tooltip and repaint the tray icon as
        // a color-coded Up/Dn meter. Only fires the Win32 update when the
        // rendered label actually changed so we don't spam NIM_MODIFY every frame.
        if let Some(tray) = self.tray.as_ref() {
            let (up_bps, dn_bps) = {
                let s = self.state.read();
                (
                    s.history_up.last().copied().unwrap_or(0.0),
                    s.history_dn.last().copied().unwrap_or(0.0),
                )
            };
            let [(up, _), (dn, _)] = self.up_dn_labels();
            let tooltip = format!("netwatch\n{up}\n{dn}");
            if tooltip != self.last_tray_tooltip {
                tray.set_tooltip(&tooltip);
                tray.set_meter_icon(up_bps, dn_bps);
                self.last_tray_tooltip = tooltip;
            }
        }

        // Pump tray + global-hotkey events once per frame.
        // Tray events are handled on a dedicated thread (see tray::spawn_event_thread).
        self.drain_hotkey_events(ctx);

        // If the user opted into "minimize to tray on close", intercept the X
        // and hide instead of quitting. Otherwise let X quit normally.
        if ctx.input(|i| i.viewport().close_requested())
            && self.tray.is_some()
            && FeatureToggle::MinimizeToTrayOnClose.get(&self.state.read())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_window(ctx);
        }

        let opacity = self.state.read().opacity;

        let (show_procs, click_through, show_background, show_title_bar) = {
            let s = self.state.read();
            (
                s.show_processes,
                s.click_through,
                s.show_background,
                s.show_title_bar,
            )
        };
        let bg_alpha = if show_background {
            (opacity * 255.0).round().clamp(0.0, 255.0) as u8
        } else {
            0
        };
        let frame = egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(15, 15, 20, bg_alpha))
            .inner_margin(egui::Margin::same(8.0));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.set_opacity(opacity);
            if !click_through {
                self.draw_titlebar(ui, ctx);
                ui.separator();
            }
            self.draw_chart(ui);
            if show_procs {
                ui.separator();
                self.draw_table(ui);
            }
        });

        if !show_title_bar && !click_through {
            self.draw_resize_grip(ctx);
        }

        self.draw_floating_menu(ctx);
        self.draw_hotkey_recording(ctx);

        // Persist settings when anything user-configurable changed this frame.
        let current = self.snapshot_settings();
        if current != self.last_saved {
            current.save();
            self.last_saved = current;
        }
    }
}

// -----------------------------------------------------------------------------
// State plumbing kept in app.rs: settings snapshot, hotkey event draining,
// window show/hide. Every user-visible widget lives under `src/ui/*.rs`.
// -----------------------------------------------------------------------------

impl NetWatchApp {
    fn snapshot_settings(&self) -> crate::settings::Settings {
        let base = crate::settings::Settings::capture_from(&self.state.read());
        let live_hotkeys = self
            .hotkeys
            .as_ref()
            .map(|reg| {
                reg.all_bindings()
                    .iter()
                    .map(|(k, v)| (k.settings_key().to_string(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        base.with_hotkeys(live_hotkeys)
    }

    fn drain_hotkey_events(&mut self, ctx: &egui::Context) {
        let Some(reg) = self.hotkeys.as_ref() else { return };
        while let Ok(ev) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
            if ev.state != global_hotkey::HotKeyState::Pressed {
                continue;
            }
            if let Some(feat) = reg.feature_for_event(ev.id) {
                let mut st = self.state.write();
                let cur = feat.get(&st);
                feat.set(&mut st, !cur, ctx, self.hwnd);
            }
        }
    }

    pub(crate) fn hide_window(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }
}

