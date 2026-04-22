//! Chart rendering: the `Plot` widget that shows the live up/dn history,
//! peak / avg horizontal overlays, current-window peak labels, and the
//! hover tooltip. All style-agnostic concerns live in `draw_chart`; the
//! series-rendering step itself dispatches through [`ChartStyle`] so new
//! visual variants can be added without touching axes, peak labels, or
//! tooltip behavior.

use eframe::egui;
use egui_plot::{BarChart, HLine, Line, Plot, PlotPoints, PlotUi};
use serde::{Deserialize, Serialize};

use crate::app::NetWatchApp;
use crate::state::{fmt_rate, HISTORY_LEN};

/// How the chart draws its two data series. Orthogonal to axes / grid / peak
/// lines — those are rendered identically for every variant so toggling the
/// style doesn't lose visual chrome.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartStyle {
    Line,
    Area,
    Bars,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self::Line
    }
}

impl ChartStyle {
    pub const ALL: &'static [Self] = &[Self::Line, Self::Area, Self::Bars];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Line => "Line",
            Self::Area => "Area",
            Self::Bars => "Bars",
        }
    }

    #[allow(dead_code)] // reserved for future settings / telemetry use
    pub fn settings_key(&self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Area => "area",
            Self::Bars => "bars",
        }
    }

    /// Render the two data series into the supplied plot. Peak/avg HLines are
    /// drawn by the caller regardless of style so they overlay cleanly on top
    /// of any variant.
    fn draw_series(
        &self,
        plot_ui: &mut PlotUi,
        up: &[f64],
        dn: &[f64],
        up_color: egui::Color32,
        dn_color: egui::Color32,
    ) {
        match self {
            Self::Line => {
                plot_ui.line(Line::new(points(dn)).color(dn_color).name("down"));
                plot_ui.line(Line::new(points(up)).color(up_color).name("up"));
            }
            Self::Area => {
                plot_ui
                    .line(Line::new(points(dn)).color(dn_color).fill(0.0).name("down"));
                plot_ui
                    .line(Line::new(points(up)).color(up_color).fill(0.0).name("up"));
            }
            Self::Bars => {
                // Interleave up/dn at each sample. Width 0.45 with offset
                // ±0.25 packs the pair to ~90% of the per-sample slot (tiny
                // gap between adjacent samples, no overlap between up and
                // dn) — the earlier 0.4/±0.2 left 50% of each slot empty
                // and produced hairline bars at 240 samples in a ~500 px
                // plot. Pixel-level legibility still depends on plot width,
                // but this is the densest no-overlap layout without
                // downsampling.
                let dn_bars: Vec<egui_plot::Bar> = dn
                    .iter()
                    .enumerate()
                    .map(|(i, v)| egui_plot::Bar::new(i as f64 - 0.25, *v).width(0.45))
                    .collect();
                let up_bars: Vec<egui_plot::Bar> = up
                    .iter()
                    .enumerate()
                    .map(|(i, v)| egui_plot::Bar::new(i as f64 + 0.25, *v).width(0.45))
                    .collect();
                plot_ui.bar_chart(BarChart::new(dn_bars).color(dn_color).name("down"));
                plot_ui.bar_chart(BarChart::new(up_bars).color(up_color).name("up"));
            }
        }
    }
}

fn points(vals: &[f64]) -> PlotPoints {
    (0..vals.len()).map(|i| [i as f64, vals[i]]).collect()
}

impl NetWatchApp {
    pub(crate) fn draw_chart(&self, ui: &mut egui::Ui) {
        // Snapshot everything the chart needs in one read so we drop the
        // lock before doing any rendering. The history vectors are cloned
        // on purpose — ETW writes land on the same `AppState` at kernel-
        // event rate (can be thousands/sec) and we don't want to starve
        // that writer while `Plot::show` runs. Two Vec<f64> × 240 ≈ 3.8 KB
        // per frame, which is cheaper than holding the read lock through a
        // full paint.
        let (up, dn, peak, mean, show_peak_avg, show_chart_axes, chart_style, show_procs) = {
            let s = self.state.read();
            let mean = if s.sample_count == 0 {
                0.0
            } else {
                s.sum_rate / s.sample_count as f64
            };
            (
                s.history_up.clone(),
                s.history_dn.clone(),
                s.peak_rate,
                mean,
                s.show_peak_avg,
                s.show_chart_axes,
                s.chart_style,
                s.show_processes,
            )
        };

        let max_y = up
            .iter()
            .chain(dn.iter())
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(if show_peak_avg { peak } else { 0.0 })
            .max(1024.0);

        // When processes are hidden, stretch the chart to fill remaining
        // height so resizing the window actually grows the graph. When
        // they're shown, keep the chart fixed and let the table take the
        // rest. Reserve a strip below the plot for the current-window peak
        // labels so they don't collide with the separator or process table
        // underneath.
        const PEAK_LABEL_STRIP: f32 = 22.0;
        let total_chart_height = if show_procs {
            crate::defaults::CHART_HEIGHT_WITH_PROCESSES
        } else {
            ui.available_height().max(crate::defaults::CHART_HEIGHT_MIN)
        };
        let chart_height = (total_chart_height - PEAK_LABEL_STRIP).max(32.0);

        let up_color = egui::Color32::from_rgb(140, 200, 255);
        let dn_color = egui::Color32::from_rgb(140, 255, 170);

        let plot_resp = Plot::new("net_chart")
            .height(chart_height)
            .show_axes([false, show_chart_axes])
            .show_grid([false, show_chart_axes])
            .show_background(false)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .include_y(0.0)
            .include_y(max_y * 1.1)
            .y_axis_formatter(|gm, _| fmt_rate(gm.value.max(0.0)))
            // Suppress built-in hover label; we paint our own below in the
            // bottom-left quadrant of the pointer.
            .label_formatter(|_, _| String::new())
            .show(ui, |plot_ui| {
                chart_style.draw_series(plot_ui, &up, &dn, up_color, dn_color);
                if show_peak_avg && peak > 0.0 {
                    plot_ui.hline(
                        HLine::new(peak)
                            .color(egui::Color32::from_rgb(255, 180, 90))
                            .name(format!("peak {}", fmt_rate(peak))),
                    );
                }
                if show_peak_avg && mean > 0.0 {
                    plot_ui.hline(
                        HLine::new(mean)
                            .color(egui::Color32::from_rgb(200, 160, 255))
                            .name(format!("avg {}", fmt_rate(mean))),
                    );
                }
            });

        // Current-window peak labels: one per direction, pinned below the
        // plot at the x of the highest sample currently in history. As
        // samples scroll out the argmax moves, so the label naturally
        // jumps to the next-highest peak.
        {
            let rect = plot_resp.response.rect;
            let label_y = rect.max.y + 1.0;
            let painter = ui.painter();
            let font = egui::FontId::proportional(11.0);
            let draw_peak = |vals: &[f64], color: egui::Color32, y_offset: f32| {
                let Some((idx, &val)) = vals
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                else {
                    return;
                };
                if val <= 0.0 {
                    return;
                }
                let peak_x = plot_resp
                    .transform
                    .position_from_point(&egui_plot::PlotPoint::new(idx as f64, 0.0))
                    .x;
                let galley = painter.layout_no_wrap(fmt_rate(val), font.clone(), color);
                let half = galley.size().x / 2.0;
                let x = (peak_x - half)
                    .max(rect.min.x + 2.0)
                    .min(rect.max.x - galley.size().x - 2.0);
                painter.galley(egui::pos2(x, label_y + y_offset), galley, color);
            };
            // Match the titlebar Up/Dn readout: Up in blue, Dn in green.
            // Keep in sync with NetWatchApp::up_dn_labels. Stagger the two
            // labels vertically so they remain legible when the peaks land
            // at or near the same x position.
            draw_peak(&up, up_color, 7.0);
            draw_peak(&dn, dn_color, -5.0);
        }
        ui.add_space(PEAK_LABEL_STRIP);

        if let Some(hover_pos) = plot_resp.response.hover_pos() {
            let plot_pos = plot_resp.transform.value_from_position(hover_pos);
            let samples_ago = ((HISTORY_LEN as f64 - 1.0) - plot_pos.x).max(0.0);
            let seconds_ago =
                samples_ago * (crate::defaults::UI_REPAINT_INTERVAL_MS as f64 / 1000.0);
            let text = format!(
                "Time = {}\nSpeed = {}",
                local_time_hms_ago(seconds_ago),
                fmt_rate(plot_pos.y.max(0.0))
            );
            let painter = ui.painter();
            let galley = painter.layout(
                text,
                egui::FontId::proportional(11.0),
                egui::Color32::from_gray(230),
                f32::INFINITY,
            );
            let size = galley.size();
            let pad = 10.0;
            // Bottom-left quadrant: text's top-right corner sits pad below/left
            // of the cursor.
            let mut pos = hover_pos + egui::vec2(-size.x - pad, pad);
            let rect = plot_resp.response.rect;
            pos.x = pos.x.max(rect.min.x + 2.0);
            pos.y = pos.y.min(rect.max.y - size.y - 2.0);
            painter.galley(pos, galley, egui::Color32::WHITE);
        }
    }
}

#[cfg(windows)]
fn local_time_hms_ago(seconds_ago: f64) -> String {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut st) };
    let sod = st.wHour as i64 * 3600 + st.wMinute as i64 * 60 + st.wSecond as i64;
    let past = (sod - seconds_ago as i64).rem_euclid(86400);
    let h = past / 3600;
    let m = (past / 60) % 60;
    let s = past % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(not(windows))]
fn local_time_hms_ago(seconds_ago: f64) -> String {
    format!("{:.0}s ago", seconds_ago)
}
