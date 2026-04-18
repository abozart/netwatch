use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use crate::defaults;

pub const HISTORY_LEN: usize = 240;

#[derive(Default, Clone)]
pub struct ProcStats {
    pub name: String,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub last_sent: u64,
    pub last_recv: u64,
    pub bucket_sent: u64,
    pub bucket_recv: u64,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SortBy {
    Pid,
    Name,
    UpRate,
    DownRate,
    UpTotal,
    DownTotal,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SortDir {
    Asc,
    Desc,
}

pub struct AppState {
    pub procs: HashMap<u32, ProcStats>,
    pub history_up: Vec<f64>,
    pub history_dn: Vec<f64>,
    pub bucket_sent: u64,
    pub bucket_recv: u64,
    pub last_tick: Instant,
    pub paused: bool,
    pub opacity: f32,
    pub always_on_top: bool,
    pub sort_by: SortBy,
    pub sort_dir: SortDir,
    pub etw_error: Option<String>,
    pub etw_started: bool,
    pub services: HashMap<u32, Vec<String>>,
    pub fw_blocked: HashSet<PathBuf>,
    pub exe_paths: HashMap<u32, PathBuf>,
    pub action_status: Option<(bool, String)>,
    pub show_processes: bool,
    pub show_title_bar: bool,
    pub click_through: bool,
    pub minimize_to_tray_on_close: bool,
    /// Feature name currently being rebound via the "Press keys…" popup.
    /// None when no recording is in progress.
    pub recording_hotkey: Option<&'static str>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            procs: HashMap::new(),
            history_up: vec![0.0; HISTORY_LEN],
            history_dn: vec![0.0; HISTORY_LEN],
            bucket_sent: 0,
            bucket_recv: 0,
            last_tick: Instant::now(),
            paused: defaults::PAUSE,
            opacity: defaults::OPACITY,
            always_on_top: defaults::ALWAYS_ON_TOP,
            sort_by: defaults::SORT_BY,
            sort_dir: defaults::SORT_DIR,
            etw_error: None,
            etw_started: false,
            services: HashMap::new(),
            fw_blocked: HashSet::new(),
            exe_paths: HashMap::new(),
            action_status: None,
            show_processes: defaults::SHOW_PROCESSES,
            show_title_bar: defaults::SHOW_TITLE_BAR,
            click_through: defaults::CLICK_THROUGH,
            minimize_to_tray_on_close: defaults::MINIMIZE_TO_TRAY_ON_CLOSE,
            recording_hotkey: None,
        }
    }

    /// Set the status banner and mirror the message to stderr so the VS Code
    /// Debug Console (and anyone launching the exe from a terminal) can see
    /// the same text the UI is showing.
    pub fn set_status(&mut self, ok: bool, msg: impl Into<String>) {
        let msg = msg.into();
        let tag = if ok { "ok" } else { "err" };
        eprintln!("[status:{tag}] {msg}");
        self.action_status = Some((ok, msg));
    }

    pub fn add_event(&mut self, pid: u32, sent: u64, recv: u64) {
        if self.paused {
            return;
        }
        let entry = self.procs.entry(pid).or_default();
        entry.bucket_sent += sent;
        entry.bucket_recv += recv;
        entry.bytes_sent += sent;
        entry.bytes_recv += recv;
        self.bucket_sent += sent;
        self.bucket_recv += recv;
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f64().max(0.001);
        self.last_tick = now;

        let up = self.bucket_sent as f64 / elapsed;
        let dn = self.bucket_recv as f64 / elapsed;
        self.history_up.remove(0);
        self.history_up.push(up);
        self.history_dn.remove(0);
        self.history_dn.push(dn);

        for p in self.procs.values_mut() {
            p.last_sent = (p.bucket_sent as f64 / elapsed) as u64;
            p.last_recv = (p.bucket_recv as f64 / elapsed) as u64;
            p.bucket_sent = 0;
            p.bucket_recv = 0;
        }
        self.bucket_sent = 0;
        self.bucket_recv = 0;
    }
}

pub fn fmt_rate(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    if bytes_per_sec >= GB {
        format!("{:.2} GB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.2} MB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.1} KB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

pub fn fmt_bytes(b: u64) -> String {
    fmt_rate(b as f64).trim_end_matches("/s").to_string()
}
