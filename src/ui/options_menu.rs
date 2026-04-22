//! Options popup contents: opacity slider, chart-style selector, 2-per-line
//! boolean toggle grid, Restart button. Invoked from the titlebar's
//! `ui.menu_button("Options", ...)`.

use eframe::egui;

use crate::app::NetWatchApp;
use crate::features::FeatureToggle;
use crate::ui::chart::ChartStyle;

impl NetWatchApp {
    /// Render the contents of the Options popup. The caller supplies the `ui`
    /// that `ui.menu_button("Options", |ui| ...)` gave us.
    pub(crate) fn draw_options_menu_contents(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
    ) {
        // Cap the menu height so it stays reachable regardless of how small
        // the window is. Menu still lives inside the hosting viewport (egui
        // constraint); a deferred-viewport popup that escapes the window is
        // an available follow-up.
        egui::ScrollArea::vertical()
            .max_height(crate::defaults::OPTIONS_MENU_MAX_HEIGHT)
            .show(ui, |ui| {
                // Opacity slider (continuous value, not a FeatureToggle).
                ui.label("Opacity");
                let mut op = self.state.read().opacity;
                if ui
                    .add(egui::Slider::new(
                        &mut op,
                        crate::defaults::OPACITY_MIN..=crate::defaults::OPACITY_MAX,
                    ))
                    .changed()
                {
                    self.state.write().opacity = op;
                }
                ui.separator();

                // Chart style selector. Iterates ChartStyle::ALL so future
                // variants slot in without touching this UI code.
                ui.horizontal(|ui| {
                    ui.label("Chart style:");
                    let mut current = self.state.read().chart_style;
                    let before = current;
                    for &style in ChartStyle::ALL {
                        ui.selectable_value(&mut current, style, style.label());
                    }
                    if current != before {
                        self.state.write().chart_style = current;
                    }
                });
                ui.separator();

                // All boolean toggles, iterated from the single source of
                // truth. Two per line; an odd-tail element spans full width
                // so it reads as a single item rather than a lonely half.
                let toggles = FeatureToggle::ALL;
                let mut i = 0;
                while i < toggles.len() {
                    if i + 1 < toggles.len() {
                        let left = toggles[i];
                        let right = toggles[i + 1];
                        ui.columns(2, |cols| {
                            self.draw_feature_row(&mut cols[0], ctx, left);
                            self.draw_feature_row(&mut cols[1], ctx, right);
                        });
                        i += 2;
                    } else {
                        self.draw_feature_row(ui, ctx, toggles[i]);
                        i += 1;
                    }
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
    }

    /// One Options-menu row: a checkbox bound to the feature's state, plus a
    /// hotkey-rebind button aligned to the right.
    pub(crate) fn draw_feature_row(
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
                ui.close_menu();
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
                    ui.close_menu();
                }
            });
        });
    }
}
