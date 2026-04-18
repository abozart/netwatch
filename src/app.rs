use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

use crate::features::FeatureToggle;
use crate::hotkeys::HotkeyRegistry;
use crate::state::{fmt_bytes, fmt_rate, AppState, SortBy, SortDir, HISTORY_LEN};
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
}

pub struct NetWatchApp {
    state: Arc<RwLock<AppState>>,
    open_menu: Option<OpenMenu>,
    last_saved: crate::settings::Settings,
    tray: Option<Tray>,
    hotkeys: Option<HotkeyRegistry>,
    hwnd: Option<isize>,
    /// Mirrors our own visibility so the tray Show/Hide item is a true toggle.
    hidden: bool,
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
                            format!("Hotkey bind failed ({}): {e}", feat.label()),
                        );
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
            hidden: false,
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
        let snapshot = self.snapshot_settings();
        snapshot.save();
        #[cfg(windows)]
        crate::etw::shutdown();
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

        // Resolve HWND on first frame so later toggles can use it.
        #[cfg(windows)]
        if self.hwnd.is_none() {
            self.hwnd = find_netwatch_hwnd();
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

        let bg_alpha = (opacity * 255.0).round().clamp(0.0, 255.0) as u8;
        let frame = egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(15, 15, 20, bg_alpha))
            .inner_margin(egui::Margin::same(8.0));

        let show_procs = self.state.read().show_processes;
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.set_opacity(opacity);
            self.draw_titlebar(ui, ctx);
            ui.separator();
            self.draw_chart(ui);
            if show_procs {
                ui.separator();
                self.draw_table(ui);
            }
        });

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

impl NetWatchApp {
    fn draw_titlebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Reserve a drag-sensitive strip for the entire top bar *before* drawing
        // widgets on top. Later widgets (buttons, menu) capture clicks normally;
        // any empty space falls through to this drag handler.
        let bar_height = 22.0;
        let bar_rect = egui::Rect::from_min_size(
            ui.cursor().min - egui::vec2(4.0, 4.0),
            egui::vec2(ui.available_width() + 8.0, bar_height + 6.0),
        );
        // Subtle lighter fill so the strip reads as a menu bar.
        ui.painter().rect_filled(
            bar_rect,
            egui::Rounding::same(2.0),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18),
        );
        let bar_drag = ui.interact(
            bar_rect,
            egui::Id::new("netwatch-titlebar-drag"),
            egui::Sense::click_and_drag(),
        );
        if bar_drag.dragged() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("netwatch")
                    .strong()
                    .color(egui::Color32::LIGHT_GRAY),
            );

            let (up, dn) = {
                let s = self.state.read();
                (
                    s.history_up.last().copied().unwrap_or(0.0),
                    s.history_dn.last().copied().unwrap_or(0.0),
                )
            };

            ui.add_space(12.0);
            let fixed_cell = |ui: &mut egui::Ui, width: f32, text: egui::RichText| {
                let h = ui.available_height();
                ui.allocate_ui_with_layout(
                    egui::vec2(width, h),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        // Force the inner ui to consume the full allocation so the
                        // parent cursor advances by exactly `width` regardless of
                        // the label's rendered length.
                        ui.set_min_size(egui::vec2(width, h));
                        ui.label(text);
                    },
                );
            };
            fixed_cell(
                ui,
                110.0,
                egui::RichText::new(format!("Up {}", fmt_rate(up)))
                    .color(egui::Color32::from_rgb(140, 200, 255)),
            );
            fixed_cell(
                ui,
                110.0,
                egui::RichText::new(format!("Dn {}", fmt_rate(dn)))
                    .color(egui::Color32::from_rgb(140, 255, 170)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("X").on_hover_text("Close").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                // Explicit minimize-to-tray button (layout is right-to-left, so
                // this renders to the left of X). Underscore matches the
                // standard Windows minimize glyph.
                if ui
                    .button("_")
                    .on_hover_text("Minimize to system tray")
                    .clicked()
                {
                    self.hide_window(ctx);
                }
                ui.menu_button("Options", |ui| {
                    // Cap the menu height so it stays reachable regardless of
                    // how small the window is. Menu still lives inside the
                    // hosting viewport (egui constraint); a deferred-viewport
                    // popup that escapes the window is an available follow-up.
                    egui::ScrollArea::vertical()
                        .max_height(crate::defaults::OPTIONS_MENU_MAX_HEIGHT)
                        .show(ui, |ui| {
                            // Opacity slider (continuous value, not a FeatureToggle).
                            ui.label("Opacity");
                            let mut op = self.state.read().opacity;
                            if ui
                                .add(egui::Slider::new(
                                    &mut op,
                                    crate::defaults::OPACITY_MIN
                                        ..=crate::defaults::OPACITY_MAX,
                                ))
                                .changed()
                            {
                                self.state.write().opacity = op;
                            }
                            ui.separator();

                            // All boolean toggles, iterated from the single source of truth.
                            for feat in FeatureToggle::ALL {
                                self.draw_feature_row(ui, ctx, *feat);
                            }

                            ui.separator();
                            if ui
                                .button("Restart")
                                .on_hover_text("Relaunch netwatch.exe")
                                .clicked()
                            {
                                if let Ok(exe) = std::env::current_exe() {
                                    let _ = std::process::Command::new(exe).spawn();
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            }
                        });
                });
            });
        });

        #[cfg(windows)]
        {
            use std::sync::atomic::Ordering;
            let seen = crate::etw::ETW_EVENTS_SEEN.load(Ordering::Relaxed);
            let matched = crate::etw::ETW_EVENTS_MATCHED.load(Ordering::Relaxed);
            let parse_err = crate::etw::ETW_EVENTS_PARSE_ERR.load(Ordering::Relaxed);
            let started = self.state.read().etw_started;
            let color = if !started || seen == 0 {
                egui::Color32::from_rgb(255, 180, 90)
            } else if matched == 0 {
                egui::Color32::from_rgb(255, 140, 90)
            } else {
                egui::Color32::from_rgb(140, 200, 140)
            };
            ui.colored_label(
                color,
                format!("ETW: started={started} seen={seen} matched={matched} parse_err={parse_err}"),
            );
        }

        if let Some(err) = self.state.read().etw_error.clone() {
            ui.colored_label(
                egui::Color32::from_rgb(255, 120, 120),
                format!("ETW: {err}"),
            );
        } else if !self.state.read().etw_started {
            ui.colored_label(egui::Color32::YELLOW, "Starting ETW trace...");
        }

        let status = self.state.read().action_status.clone();
        if let Some((ok, msg)) = status {
            let color = if ok {
                egui::Color32::from_rgb(140, 255, 170)
            } else {
                egui::Color32::from_rgb(255, 140, 140)
            };
            ui.horizontal(|ui| {
                ui.colored_label(color, &msg);
                if ui.small_button("x").clicked() {
                    self.state.write().action_status = None;
                }
            });
        }
    }

    fn draw_chart(&self, ui: &mut egui::Ui) {
        let (up, dn) = {
            let s = self.state.read();
            (s.history_up.clone(), s.history_dn.clone())
        };

        let up_pts: PlotPoints = (0..HISTORY_LEN)
            .map(|i| [i as f64, up[i]])
            .collect();
        let dn_pts: PlotPoints = (0..HISTORY_LEN)
            .map(|i| [i as f64, dn[i]])
            .collect();

        let max_y = up
            .iter()
            .chain(dn.iter())
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(1024.0);

        // When processes are hidden, stretch the chart to fill remaining
        // height so resizing the window actually grows the graph.
        // When they're shown, keep the chart fixed and let the table take
        // the rest.
        let show_procs = self.state.read().show_processes;
        let chart_height = if show_procs {
            crate::defaults::CHART_HEIGHT_WITH_PROCESSES
        } else {
            ui.available_height().max(crate::defaults::CHART_HEIGHT_MIN)
        };

        Plot::new("net_chart")
            .height(chart_height)
            .show_axes([false, true])
            .show_grid([false, true])
            .show_background(false)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .include_y(0.0)
            .include_y(max_y * 1.1)
            .y_axis_formatter(|gm, _| fmt_rate(gm.value.max(0.0)))
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(dn_pts)
                        .color(egui::Color32::from_rgb(140, 255, 170))
                        .name("down"),
                );
                plot_ui.line(
                    Line::new(up_pts)
                        .color(egui::Color32::from_rgb(140, 200, 255))
                        .name("up"),
                );
            });
    }

    fn draw_table(&mut self, ui: &mut egui::Ui) {
        let mut rows: Vec<(u32, String, u64, u64, u64, u64)> = {
            let s = self.state.read();
            s.procs
                .iter()
                .filter(|(_, p)| p.last_sent + p.last_recv > 0 || p.bytes_sent + p.bytes_recv > 0)
                .map(|(pid, p)| {
                    (
                        *pid,
                        if p.name.is_empty() {
                            format!("pid {pid}")
                        } else {
                            p.name.clone()
                        },
                        p.last_sent,
                        p.last_recv,
                        p.bytes_sent,
                        p.bytes_recv,
                    )
                })
                .collect()
        };

        let (sort_by, sort_dir) = {
            let s = self.state.read();
            (s.sort_by, s.sort_dir)
        };
        rows.sort_by(|a, b| {
            let ord = match sort_by {
                SortBy::Pid => a.0.cmp(&b.0),
                SortBy::Name => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
                SortBy::UpRate => a.2.cmp(&b.2),
                SortBy::DownRate => a.3.cmp(&b.3),
                SortBy::UpTotal => a.4.cmp(&b.4),
                SortBy::DownTotal => a.5.cmp(&b.5),
            };
            match sort_dir {
                SortDir::Asc => ord,
                SortDir::Desc => ord.reverse(),
            }
        });

        let header = |ui: &mut egui::Ui, label: &str, col: SortBy| {
            let mark = if sort_by == col {
                match sort_dir {
                    SortDir::Asc => " ^",
                    SortDir::Desc => " v",
                }
            } else {
                ""
            };
            let resp = ui.add(
                egui::Label::new(egui::RichText::new(format!("{label}{mark}")).strong())
                    .sense(egui::Sense::click()),
            );
            if resp.clicked() {
                let mut s = self.state.write();
                if s.sort_by == col {
                    s.sort_dir = match s.sort_dir {
                        SortDir::Asc => SortDir::Desc,
                        SortDir::Desc => SortDir::Asc,
                    };
                } else {
                    s.sort_by = col;
                    // numeric columns default to descending; text columns to ascending
                    s.sort_dir = match col {
                        SortBy::Name => SortDir::Asc,
                        _ => SortDir::Desc,
                    };
                }
            }
        };

        use egui_extras::{Column, TableBuilder};

        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(55.0))                       // PID
            .column(Column::remainder().at_least(120.0).clip(true)) // Process (fills)
            .column(Column::exact(80.0))                       // Up/s
            .column(Column::exact(80.0))                       // Dn/s
            .column(Column::exact(80.0))                       // Up total
            .column(Column::exact(90.0))                       // Down total
            .min_scrolled_height(0.0);

        table
            .header(20.0, |mut h| {
                h.col(|ui| header(ui, "PID", SortBy::Pid));
                h.col(|ui| header(ui, "Process", SortBy::Name));
                h.col(|ui| header(ui, "Up/s", SortBy::UpRate));
                h.col(|ui| header(ui, "Dn/s", SortBy::DownRate));
                h.col(|ui| header(ui, "Up total", SortBy::UpTotal));
                h.col(|ui| header(ui, "Down total", SortBy::DownTotal));
            })
            .body(|mut body| {
                for (pid, name, up, dn, tup, tdn) in rows.iter().take(50) {
                    let (svcs, is_blocked, exe_path) = {
                        let st = self.state.read();
                        let svcs = st.services.get(pid).cloned().unwrap_or_default();
                        let exe = st.exe_paths.get(pid).cloned();
                        let blocked = exe
                            .as_ref()
                            .map(|p| st.fw_blocked.contains(p))
                            .unwrap_or(false);
                        (svcs, blocked, exe)
                    };

                    body.row(18.0, |mut row| {
                        row.col(|ui| {
                            ui.label(format!("{pid}"));
                        });
                        row.col(|ui| {
                            let mut display = name.clone();
                            if !svcs.is_empty() {
                                display.push_str("  [svc]");
                            }
                            if is_blocked {
                                display.push_str("  [blocked]");
                            }
                            let resp = ui
                                .push_id(egui::Id::new(("proc-name", *pid)), |ui| {
                                    ui.add(
                                        egui::Label::new(display)
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                    )
                                })
                                .inner;
                            if resp.secondary_clicked() {
                                let pos = resp
                                    .interact_pointer_pos()
                                    .unwrap_or_else(|| resp.rect.left_bottom());
                                self.open_menu = Some(OpenMenu {
                                    pid: *pid,
                                    name: name.clone(),
                                    pos,
                                    exe: exe_path.clone(),
                                    services: svcs.clone(),
                                    is_blocked,
                                });
                            }
                        });
                        row.col(|ui| {
                            ui.label(fmt_rate(*up as f64));
                        });
                        row.col(|ui| {
                            ui.label(fmt_rate(*dn as f64));
                        });
                        row.col(|ui| {
                            ui.label(fmt_bytes(*tup));
                        });
                        row.col(|ui| {
                            ui.label(fmt_bytes(*tdn));
                        });
                    });
                }
            });
    }
}

impl NetWatchApp {
    fn draw_floating_menu(&mut self, ctx: &egui::Context) {
        let Some(menu) = self.open_menu.clone() else {
            return;
        };

        // ESC always closes.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open_menu = None;
            return;
        }

        let mut close = false;
        let window = egui::Window::new("netwatch-menu")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_pos(menu.pos)
            .frame(
                egui::Frame::popup(&ctx.style())
                    .fill(egui::Color32::from_rgb(30, 30, 36))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(80))),
            )
            .show(ctx, |ui| {
                ui.set_min_width(240.0);
                ui.label(
                    egui::RichText::new(format!("{}  (PID {})", menu.name, menu.pid))
                        .strong()
                        .color(egui::Color32::LIGHT_GRAY),
                );
                if let Some(exe) = menu.exe.as_ref() {
                    ui.label(
                        egui::RichText::new(exe.to_string_lossy())
                            .small()
                            .color(egui::Color32::from_gray(160)),
                    );
                }
                ui.separator();
                menu_actions(ui, &self.state, &menu, &mut close);
            });

        // Click anywhere outside the window closes it.
        if let Some(resp) = window {
            if resp.response.clicked_elsewhere() {
                close = true;
            }
        }

        if close {
            self.open_menu = None;
        }
    }
}

#[cfg(windows)]
fn menu_actions(
    ui: &mut egui::Ui,
    state: &Arc<RwLock<AppState>>,
    menu: &OpenMenu,
    close: &mut bool,
) {
    use crate::actions;

    if let Some(exe) = menu.exe.as_ref() {
        let exe_owned = exe.clone();
        if menu.is_blocked {
            if ui.button("Unblock network (firewall)").clicked() {
                dispatch(state.clone(), "Unblocked", move || {
                    actions::unblock_firewall(&exe_owned).map(|_| ())
                });
                *close = true;
            }
        } else if ui.button("Block network (firewall)").clicked() {
            dispatch(state.clone(), "Blocked in firewall", move || {
                actions::block_firewall(&exe_owned).map(|_| ())
            });
            *close = true;
        }
    } else {
        ui.label(egui::RichText::new("(no exe path — can't firewall)").weak());
    }

    if !menu.services.is_empty() {
        ui.separator();
        for svc in &menu.services {
            let name = svc.clone();
            if ui
                .button(format!("Disable service: {svc}"))
                .on_hover_text("Stop and set startup to Disabled")
                .clicked()
            {
                let n = name.clone();
                let msg = format!("Disabled {svc}");
                dispatch(state.clone(), &msg, move || {
                    actions::disable_service(&n).map(|_| ())
                });
                *close = true;
            }
            if ui.button(format!("Re-enable service: {svc}")).clicked() {
                let n = name.clone();
                let msg = format!("Re-enabled {svc}");
                dispatch(state.clone(), &msg, move || {
                    actions::enable_service(&n).map(|_| ())
                });
                *close = true;
            }
        }
    }

    ui.separator();
    let pid = menu.pid;
    if ui.button("Kill process").clicked() {
        let msg = format!("Killed PID {pid}");
        dispatch(state.clone(), &msg, move || {
            actions::kill_process(pid).map(|_| ())
        });
        *close = true;
    }

    ui.separator();
    if ui.button("Copy PID").clicked() {
        ui.ctx().copy_text(pid.to_string());
        *close = true;
    }
    if let Some(exe) = menu.exe.as_ref() {
        if ui.button("Copy exe path").clicked() {
            ui.ctx().copy_text(exe.to_string_lossy().into_owned());
            *close = true;
        }
    }
    ui.add_space(4.0);
    if ui.small_button("Close").clicked() {
        *close = true;
    }
}

#[cfg(windows)]
fn dispatch<F>(state: Arc<RwLock<AppState>>, success_msg: &str, work: F)
where
    F: FnOnce() -> anyhow::Result<()> + Send + 'static,
{
    let success_msg = success_msg.to_string();
    std::thread::spawn(move || {
        let result = work();
        let mut st = state.write();
        match result {
            Ok(()) => st.set_status(true, success_msg),
            Err(e) => st.set_status(false, format!("Failed: {e:#}")),
        }
    });
}

// -----------------------------------------------------------------------------
// Feature toggle row, tray/hotkey event handling, window show/hide, recording.
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

    fn draw_feature_row(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        feat: FeatureToggle,
    ) {
        ui.horizontal(|ui| {
            let mut val = feat.get(&self.state.read());
            if ui.checkbox(&mut val, feat.label()).changed() {
                {
                    let mut st = self.state.write();
                    feat.set(&mut st, val, ctx, self.hwnd);
                }
                // Click-through blocks further clicks from reaching the window
                // once active, so the menu would stay stuck open over the chart.
                // Explicitly close the menu for this toggle only.
                if feat == FeatureToggle::ClickThrough {
                    ui.close_menu();
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let current = self
                    .hotkeys
                    .as_ref()
                    .and_then(|r| r.binding_for(feat))
                    .map(String::from);
                let label = current.as_deref().unwrap_or("Set");
                if ui
                    .small_button(label)
                    .on_hover_text("Click to rebind hotkey")
                    .clicked()
                {
                    self.state.write().recording_hotkey = Some(feat.settings_key());
                }
            });
        });
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

    fn hide_window(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        self.hidden = true;
    }

    fn draw_hotkey_recording(&mut self, ctx: &egui::Context) {
        let Some(key) = self.state.read().recording_hotkey else { return };
        let Some(feat) = FeatureToggle::ALL.iter().copied().find(|f| f.settings_key() == key)
        else {
            self.state.write().recording_hotkey = None;
            return;
        };

        // ESC cancels.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.state.write().recording_hotkey = None;
            return;
        }

        // Grab the first non-modifier key pressed this frame, with current modifiers.
        let captured: Option<String> = ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Key { key, pressed: true, modifiers, .. } = ev {
                    if let Some(s) = combo_string(*key, *modifiers) {
                        return Some(s);
                    }
                }
            }
            None
        });

        if let Some(combo) = captured {
            if let Some(reg) = self.hotkeys.as_mut() {
                if let Err(e) = reg.bind(feat, &combo) {
                    self.state.write().set_status(
                        false,
                        format!("Could not bind {combo} for {}: {e}", feat.label()),
                    );
                } else {
                    self.state
                        .write()
                        .set_status(true, format!("Bound {combo} -> {}", feat.label()));
                }
            }
            self.state.write().recording_hotkey = None;
            return;
        }

        egui::Window::new("Press keys…")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(format!("Recording hotkey for: {}", feat.label()));
                ui.label("Press a combination (with at least one modifier).");
                ui.label("Esc to cancel.");
            });
    }
}

/// Convert an egui key + modifier set into a `global-hotkey`-compatible combo
/// string (e.g. `"Ctrl+Alt+T"`). Returns None for bare-letter presses (we
/// require a modifier so users don't accidentally grab single-letter combos)
/// and for modifier-only events.
fn combo_string(key: egui::Key, mods: egui::Modifiers) -> Option<String> {
    let has_modifier = mods.ctrl || mods.alt || mods.shift || mods.command || mods.mac_cmd;
    if !has_modifier {
        return None;
    }
    let key_name = key_to_code_name(key)?;
    let mut parts: Vec<&str> = Vec::new();
    if mods.ctrl || mods.command {
        parts.push("Ctrl");
    }
    if mods.alt {
        parts.push("Alt");
    }
    if mods.shift {
        parts.push("Shift");
    }
    if mods.mac_cmd {
        parts.push("Meta");
    }
    let mut out = parts.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&key_name);
    Some(out)
}

fn key_to_code_name(key: egui::Key) -> Option<String> {
    use egui::Key::*;
    let s: &str = match key {
        A => "KeyA", B => "KeyB", C => "KeyC", D => "KeyD", E => "KeyE",
        F => "KeyF", G => "KeyG", H => "KeyH", I => "KeyI", J => "KeyJ",
        K => "KeyK", L => "KeyL", M => "KeyM", N => "KeyN", O => "KeyO",
        P => "KeyP", Q => "KeyQ", R => "KeyR", S => "KeyS", T => "KeyT",
        U => "KeyU", V => "KeyV", W => "KeyW", X => "KeyX", Y => "KeyY",
        Z => "KeyZ",
        Num0 => "Digit0", Num1 => "Digit1", Num2 => "Digit2", Num3 => "Digit3",
        Num4 => "Digit4", Num5 => "Digit5", Num6 => "Digit6", Num7 => "Digit7",
        Num8 => "Digit8", Num9 => "Digit9",
        F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6",
        F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
        Space => "Space",
        ArrowLeft => "ArrowLeft", ArrowRight => "ArrowRight",
        ArrowUp => "ArrowUp", ArrowDown => "ArrowDown",
        _ => return None,
    };
    Some(s.to_string())
}

#[cfg(windows)]
fn find_netwatch_hwnd() -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    let title: Vec<u16> = "netwatch"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        None
    } else {
        Some(hwnd as isize)
    }
}
