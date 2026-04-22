use anyhow::Result;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Total ETW callback invocations since startup (all event IDs, all parse outcomes).
pub static ETW_EVENTS_SEEN: AtomicU64 = AtomicU64::new(0);
/// Events that matched one of our four event IDs and successfully parsed PID+size.
pub static ETW_EVENTS_MATCHED: AtomicU64 = AtomicU64::new(0);
/// Events we saw but couldn't get a schema/parser for (prod/schema mismatch).
pub static ETW_EVENTS_PARSE_ERR: AtomicU64 = AtomicU64::new(0);

/// Flipped by [`shutdown`] so the sibling tick thread spawned in [`run`]
/// exits its polling loop instead of surviving past the ETW trace stop.
/// Harmless today because `on_exit` calls `std::process::exit`, but without
/// this flag the thread would leak if the force-exit path ever goes away.
static TICK_STOP: AtomicBool = AtomicBool::new(false);

use ferrisetw::parser::Parser;
use ferrisetw::provider::Provider;
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::{TraceTrait, UserTrace};
use ferrisetw::EventRecord;

use crate::state::AppState;

const KERNEL_NETWORK_GUID: &str = "7DD42A49-5329-4832-8DFD-43D979153A88";
const SESSION_PREFIX: &str = "netwatch-etw";

static CURRENT_SESSION: parking_lot::Mutex<Option<String>> = parking_lot::Mutex::new(None);

fn session_name() -> String {
    format!("{SESSION_PREFIX}-{}", std::process::id())
}

pub fn shutdown() {
    TICK_STOP.store(true, Ordering::Relaxed);
    if let Some(name) = CURRENT_SESSION.lock().take() {
        stop_existing_session(&name);
    }
    // Best-effort cleanup of any leaked PID-named sessions.
    stop_all_netwatch_sessions();
}

/// Enumerate every active ETW session and stop each one whose name starts with
/// `netwatch-etw`. Covers the legacy unnamed session and all per-PID leaks
/// from prior crashed runs.
fn stop_all_netwatch_sessions() {
    use windows_sys::Win32::System::Diagnostics::Etw::{
        QueryAllTracesW, EVENT_TRACE_PROPERTIES,
    };

    const MAX_SESSIONS: usize = 64;
    const PROPS_SIZE: usize = std::mem::size_of::<EVENT_TRACE_PROPERTIES>();
    const NAME_BYTES: usize = 1024;
    const LOG_BYTES: usize = 1024;
    const BUF_SIZE: usize = PROPS_SIZE + NAME_BYTES + LOG_BYTES;

    // Each slot needs its own contiguous PROPERTIES + name + logfile buffer.
    let mut slots: Vec<Vec<u8>> = (0..MAX_SESSIONS).map(|_| vec![0u8; BUF_SIZE]).collect();
    let mut ptrs: Vec<*mut EVENT_TRACE_PROPERTIES> = Vec::with_capacity(MAX_SESSIONS);
    for slot in slots.iter_mut() {
        let p = slot.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        unsafe {
            (*p).Wnode.BufferSize = BUF_SIZE as u32;
            (*p).LoggerNameOffset = PROPS_SIZE as u32;
            (*p).LogFileNameOffset = (PROPS_SIZE + NAME_BYTES) as u32;
        }
        ptrs.push(p);
    }

    let mut count: u32 = 0;
    let rc = unsafe { QueryAllTracesW(ptrs.as_mut_ptr(), MAX_SESSIONS as u32, &mut count) };
    if rc != 0 {
        return; // nothing to do; best-effort
    }

    for &p in ptrs.iter().take(count as usize) {
        let name_ptr = unsafe { (p as *const u8).add(PROPS_SIZE) as *const u16 };
        let name = read_wide_nul(name_ptr, NAME_BYTES / 2);
        if name.starts_with(SESSION_PREFIX) {
            eprintln!("[etw] stopping leaked session: {name}");
            stop_existing_session(&name);
        }
    }
}

fn read_wide_nul(ptr: *const u16, max_chars: usize) -> String {
    let mut len = 0usize;
    unsafe {
        while len < max_chars && *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16_lossy(slice)
    }
}

fn stop_existing_session(name: &str) {
    use windows_sys::Win32::System::Diagnostics::Etw::{
        ControlTraceW, CONTROLTRACE_HANDLE, EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_PROPERTIES,
    };
    const NAME_BYTES: usize = 1024;
    const LOG_BYTES: usize = 1024;
    const BUF_SIZE: usize = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + NAME_BYTES + LOG_BYTES;

    let mut buf = vec![0u8; BUF_SIZE];
    unsafe {
        let props = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        (*props).Wnode.BufferSize = BUF_SIZE as u32;
        (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        (*props).LogFileNameOffset = (*props).LoggerNameOffset + NAME_BYTES as u32;

        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = ControlTraceW(
            CONTROLTRACE_HANDLE { Value: 0 },
            wide.as_ptr(),
            props,
            EVENT_TRACE_CONTROL_STOP,
        );
    }
}

pub fn run(state: Arc<RwLock<AppState>>) -> Result<()> {
    eprintln!("[etw] run() entered");
    // Tick + process-name backfill on a sibling thread.
    let tick_state = state.clone();
    std::thread::spawn(move || {
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        loop {
            // Poll the shutdown flag on a shorter cadence than the refresh
            // so `etw::shutdown` takes effect within ~100 ms instead of
            // waiting out a full second of sleep.
            for _ in 0..10 {
                if TICK_STOP.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let alive: std::collections::HashSet<u32> = sys
                .processes()
                .keys()
                .map(|p| p.as_u32())
                .collect();
            let mut st = tick_state.write();
            // Prune anything the OS no longer shows (e.g. just-killed process).
            st.procs.retain(|pid, _| alive.contains(pid));
            st.exe_paths.retain(|pid, _| alive.contains(pid));

            let pids: Vec<u32> = st.procs.keys().copied().collect();
            for pid in pids {
                if let Some(proc) = sys.process(Pid::from_u32(pid)) {
                    if let Some(p) = st.procs.get_mut(&pid) {
                        if p.name.is_empty() {
                            p.name = proc.name().to_string_lossy().to_string();
                        }
                    }
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        st.exe_paths.entry(pid)
                    {
                        if let Some(path) = proc.exe() {
                            e.insert(path.to_path_buf());
                        }
                    }
                }
            }
            st.tick();
        }
    });

    let cb_state = state.clone();
    let provider = Provider::by_guid(KERNEL_NETWORK_GUID)
        // MatchAnyKeyword: 0 means "deliver no events" for manifest providers,
        // so we request all keyword bits to get every TCP/UDP send/recv.
        .any(u64::MAX)
        .add_callback(move |record: &EventRecord, locator: &SchemaLocator| {
            on_event(record, locator, &cb_state);
        })
        .build();

    // Enumerate every active session and stop any that belong to us (prior
    // crashed PID-named sessions plus the legacy unnamed one). Prevents the
    // "multiple sessions competing for the same provider events" failure mode.
    stop_all_netwatch_sessions();
    stop_existing_session(SESSION_PREFIX);

    let name = session_name();
    eprintln!("[etw] starting session {name}");
    *CURRENT_SESSION.lock() = Some(name.clone());

    let (_trace, handle) = UserTrace::new()
        .named(name)
        .enable(provider)
        .start()
        .map_err(|e| anyhow::anyhow!(
            "failed to start ETW user trace: {e:?}\n\
             Run once in elevated PowerShell to grant access without UAC:\n\
             Add-LocalGroupMember -Group 'Performance Log Users' -Member $env:USERNAME\n\
             then sign out and back in.\n\
             If error is AlreadyExist, also run: logman stop netwatch-etw -ets"
        ))?;

    state.write().etw_started = true;

    UserTrace::process_from_handle(handle)
        .map_err(|e| anyhow::anyhow!("ETW processing loop ended: {e:?}"))?;
    Ok(())
}

fn on_event(record: &EventRecord, locator: &SchemaLocator, state: &Arc<RwLock<AppState>>) {
    ETW_EVENTS_SEEN.fetch_add(1, Ordering::Relaxed);

    let schema = match locator.event_schema(record) {
        Ok(s) => s,
        Err(_) => {
            ETW_EVENTS_PARSE_ERR.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };
    let parser = Parser::create(record, &schema);

    let pid: u32 = match parser.try_parse("PID") {
        Ok(v) => v,
        Err(_) => {
            ETW_EVENTS_PARSE_ERR.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };
    let size: u32 = match parser.try_parse("size") {
        Ok(v) => v,
        Err(_) => {
            ETW_EVENTS_PARSE_ERR.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };

    // Microsoft-Windows-Kernel-Network event IDs:
    //   10 = TcpDataSent, 11 = TcpDataRecv
    //   26 = UdpDataSent, 27 = UdpDataRecv
    let id = record.event_id();
    let (sent, recv) = match id {
        10 | 26 => (size as u64, 0),
        11 | 27 => (0, size as u64),
        _ => return,
    };

    ETW_EVENTS_MATCHED.fetch_add(1, Ordering::Relaxed);
    state.write().add_event(pid, sent, recv);
}
