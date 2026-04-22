//! Custom titlebar: drag region, app name, live Up/Dn readout, Options
//! menu button, minimize-to-tray, close. Used when `ShowTitleBar` is off
//! (native chrome hidden) — and even when it's on, this bar still renders
//! inside the panel because the OS titlebar doesn't show Up/Dn.

use eframe::egui;

use crate::app::NetWatchApp;
use crate::state::fmt_rate;

impl NetWatchApp {
    /// Shared source of the "Up X B/s" / "Dn X B/s" summary shown in both
    /// the custom titlebar and the tray tooltip. Returned as (text, color)
    /// pairs so each call site can render with its own layout (widgets vs
    /// painter) without duplicating the formatting or color choices.
    pub(crate) fn up_dn_labels(&self) -> [(String, egui::Color32); 2] {
        let (up, dn) = {
            let s = self.state.read();
            (
                s.history_up.last().copied().unwrap_or(0.0),
                s.history_dn.last().copied().unwrap_or(0.0),
            )
        };
        [
            (
                format!("Up {}", fmt_rate(up)),
                egui::Color32::from_rgb(140, 200, 255),
            ),
            (
                format!("Dn {}", fmt_rate(dn)),
                egui::Color32::from_rgb(140, 255, 170),
            ),
        ]
    }

    pub(crate) fn draw_titlebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Reserve a drag-sensitive strip for the entire top bar *before* drawing
        // widgets on top. Later widgets (buttons, menu) capture clicks normally;
        // any empty space falls through to this drag handler.
        let bar_height = 22.0;
        let bar_rect = egui::Rect::from_min_size(
            ui.cursor().min - egui::vec2(4.0, 4.0),
            egui::vec2(ui.available_width() + 8.0, bar_height + 6.0),
        );
        // Solid gray fill — darker than a white-tinted strip, still lighter than
        // the app background so it reads as a menu bar.
        ui.painter().rect_filled(
            bar_rect,
            egui::Rounding::same(2.0),
            egui::Color32::from_gray(42),
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
            for (text, color) in self.up_dn_labels() {
                fixed_cell(ui, 110.0, egui::RichText::new(text).color(color));
            }

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
                    self.draw_options_menu_contents(ui, ctx);
                });
            });
        });

        // Verbose ETW counters are noisy for end users — only show in debug
        // builds (or when something looks wrong below). The etw_error path
        // still surfaces real failures.
        #[cfg(all(windows, debug_assertions))]
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
}
