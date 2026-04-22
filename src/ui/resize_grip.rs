//! Custom bottom-right resize handle. Only shown when the native titlebar
//! is hidden — otherwise the OS chrome already provides one. Dragging it
//! hands the resize off to the OS via `ViewportCommand::BeginResize`.

use eframe::egui;

use crate::app::NetWatchApp;

impl NetWatchApp {
    pub(crate) fn draw_resize_grip(&self, ctx: &egui::Context) {
        let screen_rect = ctx.input(|i| i.screen_rect());
        let grip_size = 14.0;
        let grip_rect = egui::Rect::from_min_size(
            egui::pos2(
                screen_rect.max.x - grip_size,
                screen_rect.max.y - grip_size,
            ),
            egui::vec2(grip_size, grip_size),
        );

        egui::Area::new(egui::Id::new("netwatch-resize-grip"))
            .fixed_pos(grip_rect.min)
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                let resp = ui.allocate_response(grip_rect.size(), egui::Sense::drag());
                if resp.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeSouthEast);
                }
                if resp.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                        egui::ResizeDirection::SouthEast,
                    ));
                }
                // Three short diagonal hash marks, classic Windows grip look.
                let painter = ui.painter();
                let color = egui::Color32::from_gray(160);
                let stroke = egui::Stroke::new(1.0, color);
                for i in 0..3 {
                    let off = 3.0 + i as f32 * 4.0;
                    painter.line_segment(
                        [
                            egui::pos2(resp.rect.max.x - off, resp.rect.max.y - 2.0),
                            egui::pos2(resp.rect.max.x - 2.0, resp.rect.max.y - off),
                        ],
                        stroke,
                    );
                }
            });
    }
}
