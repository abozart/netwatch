//! Process table: sorted list of PIDs with their names, live up/dn rates,
//! cumulative totals, and right-click context menu to open firewall /
//! service / scheduled-task actions.

use eframe::egui;
use std::sync::Arc;

use crate::app::{NetWatchApp, OpenMenu};
use crate::state::{fmt_bytes, fmt_rate, SortBy, SortDir};

impl NetWatchApp {
    pub(crate) fn draw_table(&mut self, ui: &mut egui::Ui) {
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

        // Bump the alternating-row tint well above egui's default so the
        // stripes actually read on our dark semi-transparent background.
        // Scoped to this ui; does not affect widgets drawn afterward.
        ui.visuals_mut().faint_bg_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14);

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
                                #[cfg(windows)]
                                let tasks =
                                    Arc::new(parking_lot::Mutex::new(Vec::new()));
                                #[cfg(windows)]
                                if let Some(exe) = exe_path.clone() {
                                    let slot = tasks.clone();
                                    let ctx_wake = ui.ctx().clone();
                                    std::thread::spawn(move || {
                                        if let Ok(found) = crate::tasks::find_tasks_for(&exe) {
                                            *slot.lock() = found;
                                            ctx_wake.request_repaint();
                                        }
                                    });
                                }
                                self.open_menu = Some(OpenMenu {
                                    pid: *pid,
                                    name: name.clone(),
                                    pos,
                                    exe: exe_path.clone(),
                                    services: svcs.clone(),
                                    is_blocked,
                                    #[cfg(windows)]
                                    tasks,
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
