//! Right-click context menu opened against a table row. Buttons here fan
//! out to firewall, service, scheduled-task, and kill-process actions via
//! `crate::actions`. Long-running work runs on a worker thread so the UI
//! stays responsive; results come back through `AppState::set_status`.

use eframe::egui;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::app::{NetWatchApp, OpenMenu};
use crate::state::AppState;

impl NetWatchApp {
    pub(crate) fn draw_floating_menu(&mut self, ctx: &egui::Context) {
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

    let tasks_snapshot = menu.tasks.lock().clone();
    if !tasks_snapshot.is_empty() {
        ui.separator();
        for task in &tasks_snapshot {
            let path = task.full_path.clone();
            // Show the leaf name (full path is in the hover tooltip).
            let leaf = path.rsplit('\\').next().unwrap_or(path.as_str());
            if ui
                .button(format!("Disable scheduled task: {leaf}"))
                .on_hover_text(&path)
                .clicked()
            {
                let p = path.clone();
                let msg = format!("Disabled task {leaf}");
                dispatch(state.clone(), &msg, move || {
                    actions::disable_scheduled_task(&p).map(|_| ())
                });
                *close = true;
            }
            if ui
                .button(format!("Re-enable scheduled task: {leaf}"))
                .on_hover_text(&path)
                .clicked()
            {
                let p = path.clone();
                let msg = format!("Re-enabled task {leaf}");
                dispatch(state.clone(), &msg, move || {
                    actions::enable_scheduled_task(&p).map(|_| ())
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

#[cfg(not(windows))]
fn menu_actions(
    ui: &mut egui::Ui,
    _state: &Arc<RwLock<AppState>>,
    _menu: &OpenMenu,
    close: &mut bool,
) {
    ui.label(
        egui::RichText::new("Process actions are Windows-only.")
            .weak(),
    );
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
