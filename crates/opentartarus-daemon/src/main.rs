use opentartarus_core::constants::USB_VID_RAZER;
use opentartarus_core::error::ErrorCode;
use opentartarus_core::ipc::{EventMethod, Method};
use opentartarus_core::keymap::{mouse_button_to_evdev, EV_KEY, EV_REL, REL_WHEEL};
use opentartarus_core::lighting::LightingClient;
use opentartarus_core::pack::SHIPPED_IDS;
use opentartarus_core::paths::Paths;
use opentartarus_core::record::RecordReason;
use opentartarus_core::remap::{Clock, RawEvent, RemapEngine};
use opentartarus_core::store::read_active_id;
use opentartarus_core::types::{DeviceModel, MouseButton, ScrollDir};
use opentartarus_daemon::device::{
    busy_decision, classify_device_io, detect_status, device_ui_state, enumerate_tartarus,
    grab_conflict_status_value, grab_targets, lookup_event_holders, pick_first, scan_interval_ms,
    snapshot_changed, BusyDecision, DetectStatus, Detected, DeviceIoKind, DeviceUiState,
};
use opentartarus_daemon::handler::{handle_request, DaemonState};
use opentartarus_daemon::perms::{probe_evdev_readable, probe_uinput};
use opentartarus_daemon::playback::{
    commit_if_engine_replaced, remap_physical_event, tick_engine, EngineEpoch,
};
use opentartarus_daemon::server::{accept_loop, bind_exclusive, emit_event, listen_error_code};
use opentartarus_daemon::spawn_ui::UiSupervisor;
use opentartarus_daemon::tray::{
    spawn_tray, tray_name_from_applied_params, tray_name_from_profile_id, TrayCmd,
};
use opentartarus_daemon::uinput_sink::{token_from_evdev, UinputSink};
use opentartarus_daemon::{daemon_lighting, set_daemon_lighting_usb, DaemonLighting};
use opentartarus_daemon::{log, tray};
use serde_json::json;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const DEFAULT_PROFILE_NAME: &str = "Default";
const POLL_INTERVAL_MS: u64 = 250;
const POLL_INTERVAL: Duration = Duration::from_millis(POLL_INTERVAL_MS);
const TICK_INTERVAL_MS: u64 = 10;
const TICK_INTERVAL: Duration = Duration::from_millis(TICK_INTERVAL_MS);
const KEY_DOWN: i32 = 1;
const EV_SYN: u16 = 0;
const ALREADY_RUNNING_EXIT: i32 = 1;
const SUCCESS_EXIT: i32 = 0;

struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        now_ms()
    }

    fn sleep_ms(&mut self, ms: u64) {
        if ms > 0 {
            std::thread::sleep(Duration::from_millis(ms));
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn active_profile_name<L: LightingClient>(state: &DaemonState<L>) -> String {
    state
        .active_id
        .as_deref()
        .map(tray_name_from_profile_id)
        .unwrap_or_else(|| DEFAULT_PROFILE_NAME.to_string())
}

const USB_ID_HEX_WIDTH: usize = 4;

fn usb_id_hex(id: u16) -> String {
    format!("{id:0width$x}", width = USB_ID_HEX_WIDTH)
}

fn device_event_params<L: LightingClient>(state: &DaemonState<L>) -> serde_json::Value {
    if !state.device_present {
        return json!({ "present": false, "model": null, "vid": null, "pid": null });
    }
    match state.model {
        Some(model) => json!({
            "present": true,
            "model": model,
            "vid": usb_id_hex(USB_VID_RAZER),
            "pid": usb_id_hex(model.usb_pid()),
        }),
        None => json!({ "present": true, "model": null, "vid": null, "pid": null }),
    }
}

#[tokio::main]
async fn main() {
    let code = match run().await {
        Ok(()) => SUCCESS_EXIT,
        Err(ErrorCode::AlreadyRunning) => ALREADY_RUNNING_EXIT,
        Err(_) => ALREADY_RUNNING_EXIT,
    };
    std::process::exit(code);
}

async fn run() -> Result<(), ErrorCode> {
    let paths = Paths::from_env();
    log::init(&paths.log_file);

    let listener = match bind_exclusive(&paths.socket) {
        Ok(listener) => listener,
        Err(err) => {
            let code = listen_error_code(&err);
            log::write(&code.log_line(Some(&err.to_string())));
            return Err(code);
        }
    };

    let mut uinput_ok = probe_uinput();
    let sink = if uinput_ok {
        match UinputSink::open() {
            Ok(sink) => Some(sink),
            Err(err) => {
                log::write(&err.log_line(None));
                uinput_ok = false;
                None
            }
        }
    } else {
        None
    };

    let found = pick_first(enumerate_tartarus());
    let status = detect_status(found.as_ref(), probe_evdev_readable);
    let snapshot = device_ui_state(status, found.as_ref().map(|detected| detected.model), false);
    let device_present = snapshot.present;
    let model = snapshot.model;
    let evdev_ok = snapshot.evdev_ok;
    let grab_conflict = None;
    let vid = found.as_ref().map(|detected| detected.vid);
    let pid = found.as_ref().map(|detected| detected.pid);

    let lighting = daemon_lighting(vid, pid);
    let openrazer_available = lighting.available();
    let mut state = DaemonState {
        paths: paths.clone(),
        engine: RemapEngine::new(model.unwrap_or(DeviceModel::V2)),
        recorder: Default::default(),
        lighting,
        active_id: None,
        device_present,
        model,
        openrazer_available,
        evdev_ok,
        uinput_ok,
        grab_conflict,
    };

    if let Ok(Some(id)) = read_active_id(&paths) {
        if SHIPPED_IDS.contains(&id.as_str()) {
            let _ = handle_request(
                &mut state,
                Method::ApplyProfile,
                json!({ "id": id }),
                now_ms(),
            );
        }
    }

    let profile_name = active_profile_name(&state);
    let state = Arc::new(Mutex::new(state));
    let sink = Arc::new(Mutex::new(sink));
    let quit = Arc::new(AtomicBool::new(false));
    let clients = Arc::new(AtomicUsize::new(0));
    let epoch = Arc::new(EngineEpoch::new());
    let (event_tx, _) = broadcast::channel(64);
    let mut events_rx = event_tx.subscribe();
    let (tray_tx, mut tray_rx) = tokio::sync::mpsc::unbounded_channel();

    let tray_icon = tray::OpenTartarusTray::new(profile_name, tray_tx.clone());
    let tray_handle = spawn_tray(tray_icon).await;
    let _tray_keep_alive = tray_tx;

    {
        let listener_state = Arc::clone(&state);
        let events = event_tx.clone();
        let quit_flag = Arc::clone(&quit);
        let client_count = Arc::clone(&clients);
        let epoch = Arc::clone(&epoch);
        tokio::spawn(async move {
            accept_loop(
                listener,
                listener_state,
                events,
                quit_flag,
                client_count,
                epoch,
            )
            .await;
        });
    }

    {
        let device_state = Arc::clone(&state);
        let device_sink = Arc::clone(&sink);
        let events = event_tx.clone();
        let quit_flag = Arc::clone(&quit);
        let epoch = Arc::clone(&epoch);
        std::thread::spawn(move || {
            device_loop(device_state, device_sink, events, quit_flag, epoch);
        });
    }

    let mut ui = UiSupervisor::new();
    loop {
        tokio::select! {
            cmd = tray_rx.recv() => {
                match cmd {
                    Some(TrayCmd::Quit) => {
                        request_quit(&state, &event_tx, &quit);
                        break;
                    }
                    None => {}
                    Some(TrayCmd::Open) => {
                        let connected = clients.load(Ordering::SeqCst) > 0;
                        if ui.ensure_open(connected) {
                            emit_event(&event_tx, EventMethod::FocusWindow, json!({}));
                        }
                    }
                    Some(TrayCmd::ApplyProfile(id)) => {
                        let applied = {
                            let mut st = state.lock().expect("daemon state");
                            let result = handle_request(
                                &mut st,
                                Method::ApplyProfile,
                                json!({ "id": id }),
                                now_ms(),
                            );
                            commit_if_engine_replaced(
                                result.is_ok(),
                                Some(Method::ApplyProfile),
                                &epoch,
                            );
                            result.is_ok()
                        };
                        if applied {
                            emit_event(
                                &event_tx,
                                EventMethod::ProfileApplied,
                                json!({ "id": id }),
                            );
                        }
                    }
                }
            }
            event = events_rx.recv() => {
                match event {
                    Ok(msg) if msg.method == EventMethod::ProfileApplied => {
                        if let Some(name) = tray_name_from_applied_params(&msg.params) {
                            if let Some(handle) = &tray_handle {
                                handle
                                    .update(|tray_state| tray_state.profile_name = name)
                                    .await;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let name = {
                            let st = state.lock().expect("daemon state");
                            active_profile_name(&st)
                        };
                        if let Some(handle) = &tray_handle {
                            handle
                                .update(|tray_state| tray_state.profile_name = name)
                                .await;
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = tokio::signal::ctrl_c() => {
                request_quit(&state, &event_tx, &quit);
                break;
            }
        }
        if quit.load(Ordering::SeqCst) {
            break;
        }
    }

    quit.store(true, Ordering::SeqCst);
    emit_event(&event_tx, EventMethod::DaemonStopping, json!({}));
    {
        let mut held = sink.lock().expect("uinput sink");
        *held = None;
    }
    let _ = std::fs::remove_file(&paths.socket);
    Ok(())
}

fn request_quit<L: LightingClient>(
    state: &Mutex<DaemonState<L>>,
    events: &broadcast::Sender<opentartarus_core::ipc::EventMsg>,
    quit: &AtomicBool,
) {
    if let Ok(mut st) = state.lock() {
        let _ = handle_request(&mut st, Method::QuitDaemon, json!({}), now_ms());
    }
    emit_event(events, EventMethod::DaemonStopping, json!({}));
    quit.store(true, Ordering::SeqCst);
}

fn device_loop(
    state: Arc<Mutex<DaemonState<DaemonLighting>>>,
    sink: Arc<Mutex<Option<UinputSink>>>,
    events: broadcast::Sender<opentartarus_core::ipc::EventMsg>,
    quit: Arc<AtomicBool>,
    epoch: Arc<EngineEpoch>,
) {
    let mut grabbed: Vec<evdev::Device> = Vec::new();
    let mut grabbed_ids: Option<(u16, u16, DeviceModel)> = None;
    let mut grabbed_nodes: Vec<PathBuf> = Vec::new();
    // Indexes into `grabbed` of gamepad nodes, and whether they are held.
    let mut gamepad_idx: Vec<usize> = Vec::new();
    let mut gamepad_held = false;
    let mut last_ui: Option<DeviceUiState> = None;

    while !quit.load(Ordering::SeqCst) {
        poll_record_timeout(&state, &events);

        if grabbed.is_empty() {
            let detected = pick_first(enumerate_tartarus());
            let status = detect_status(detected.as_ref(), probe_evdev_readable);
            match (status, detected) {
                (DetectStatus::Present, Some(detected)) => {
                    match grab_nodes(grab_targets(&detected)) {
                        Ok(devices) => {
                            grabbed = devices;
                            grabbed_ids = Some((detected.vid, detected.pid, detected.model));
                            grabbed_nodes = grab_targets(&detected).to_vec();
                            gamepad_idx = grabbed_nodes
                                .iter()
                                .enumerate()
                                .filter(|(_, path)| detected.gamepad_nodes.contains(path))
                                .map(|(index, _)| index)
                                .collect();
                            gamepad_held = true;
                            last_ui = Some(device_ui_state(
                                DetectStatus::Present,
                                Some(detected.model),
                                false,
                            ));
                            on_device_appeared(&state, &detected, &events, &epoch);
                        }
                        Err(fail) => {
                            let grab_conflict = fail.code == ErrorCode::GrabConflict;
                            let next = device_ui_state(
                                DetectStatus::Present,
                                Some(detected.model),
                                grab_conflict,
                            );
                            publish_idle_detect(
                                &state,
                                &events,
                                &mut last_ui,
                                next,
                                Some(detected.vid),
                                Some(detected.pid),
                                Some(fail),
                            );
                        }
                    }
                }
                (DetectStatus::Permission, Some(detected)) => {
                    let next =
                        device_ui_state(DetectStatus::Permission, Some(detected.model), false);
                    publish_idle_detect(
                        &state,
                        &events,
                        &mut last_ui,
                        next,
                        Some(detected.vid),
                        Some(detected.pid),
                        Some(GrabFail {
                            code: ErrorCode::Permission,
                            os: String::new(),
                            holder: None,
                        }),
                    );
                }
                (DetectStatus::Missing, _)
                | (DetectStatus::Permission, None)
                | (DetectStatus::Present, None) => {
                    let next = device_ui_state(DetectStatus::Missing, None, false);
                    if last_ui.as_ref().is_some_and(|snap| snap.present)
                        && last_ui
                            .as_ref()
                            .is_some_and(|prev| snapshot_changed(prev, &next))
                    {
                        on_device_vanished(&state, &events);
                    }
                    last_ui = Some(next);
                }
            }
            std::thread::sleep(Duration::from_millis(scan_interval_ms(false)));
            continue;
        }

        if !grabbed_nodes.iter().all(|path| path.exists()) {
            grabbed.clear();
            grabbed_ids = None;
            grabbed_nodes.clear();
            last_ui = Some(device_ui_state(DetectStatus::Missing, None, false));
            on_device_vanished(&state, &events);
            continue;
        }

        let Some((vid, pid, _)) = grabbed_ids else {
            grabbed.clear();
            continue;
        };

        if !gamepad_idx.is_empty() {
            let want = state.lock().expect("daemon state").engine.binds_stick();
            if want != gamepad_held {
                hold_gamepads(&mut grabbed, &gamepad_idx, want);
                gamepad_held = want;
            }
        }

        let mut pending = Vec::new();
        let mut disconnect = false;
        for device in &mut grabbed {
            match device.fetch_events() {
                Ok(iter) => {
                    for ev in iter {
                        pending.push((ev.event_type().0, ev.code(), ev.value()));
                    }
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {}
                Err(_) => {
                    disconnect = true;
                    break;
                }
            }
        }
        if disconnect {
            grabbed.clear();
            grabbed_ids = None;
            grabbed_nodes.clear();
            last_ui = Some(device_ui_state(DetectStatus::Missing, None, false));
            on_device_vanished(&state, &events);
            continue;
        }
        if pending.is_empty() {
            tick_remap(&state, &sink, &epoch);
            wait_for_input(&grabbed, idle_sleep(&state));
            continue;
        }
        for (ev_type, code, value) in pending {
            if ev_type == EV_SYN {
                continue;
            }
            handle_physical_event(
                &state,
                &sink,
                &events,
                &epoch,
                RawEvent {
                    vid,
                    pid,
                    ev_type,
                    code,
                    value,
                },
            );
        }
        tick_remap(&state, &sink, &epoch);
    }
}

fn tick_remap(
    state: &Mutex<DaemonState<DaemonLighting>>,
    sink: &Mutex<Option<UinputSink>>,
    epoch: &EngineEpoch,
) {
    let mut clock = SystemClock;
    tick_engine(state, sink, epoch, &mut clock);
}

/// Blocks until one of the pad's nodes has events to read, or `timeout`
/// passes. The timeout is only the longest wait, for the checks the loop does
/// between reads (unplug, a profile that now binds the stick, macro timers).
/// Waking on input instead of on a timer matters most for the thumbstick: it
/// sends hundreds of events a second, and a reader that sleeps through them
/// both lags and overflows the kernel's buffer, losing the stick's return to
/// the centre.
fn wait_for_input(devices: &[evdev::Device], timeout: Duration) {
    use std::os::fd::AsRawFd;
    let mut fds: Vec<libc::pollfd> = devices
        .iter()
        .map(|device| libc::pollfd {
            fd: device.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    let timeout_ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: `fds` is a valid, initialised array of `fds.len()` pollfd
    // structs that outlives the call, and every fd in it stays open because
    // `devices` is borrowed for the whole call. A failed or interrupted poll
    // only means the loop reads again sooner.
    unsafe {
        libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout_ms);
    }
}

fn idle_sleep(state: &Mutex<DaemonState<DaemonLighting>>) -> Duration {
    let st = state.lock().expect("daemon state");
    if st.engine.has_timed_work() {
        TICK_INTERVAL
    } else {
        POLL_INTERVAL
    }
}

fn publish_idle_detect(
    state: &Mutex<DaemonState<DaemonLighting>>,
    events: &broadcast::Sender<opentartarus_core::ipc::EventMsg>,
    last: &mut Option<DeviceUiState>,
    next: DeviceUiState,
    vid: Option<u16>,
    pid: Option<u16>,
    error: Option<GrabFail>,
) {
    if last
        .as_ref()
        .is_some_and(|prev| !snapshot_changed(prev, &next))
    {
        return;
    }
    let mut st = state.lock().expect("daemon state");
    st.device_present = next.present;
    st.model = next.model;
    st.evdev_ok = next.evdev_ok;
    st.grab_conflict = if next.grab_conflict {
        Some(grab_conflict_status_value(
            error.as_ref().and_then(|fail| fail.holder.as_deref()),
        ))
    } else {
        None
    };
    match (vid, pid) {
        (Some(vid), Some(pid)) if next.present => {
            update_lighting_usb(&mut st.lighting, Some((vid, pid)))
        }
        _ => update_lighting_usb(&mut st.lighting, None),
    }
    st.openrazer_available = st.lighting.available();
    emit_event(events, EventMethod::DeviceChanged, device_event_params(&st));
    if let Some(fail) = error {
        let os = match (&fail.holder, fail.os.is_empty()) {
            (Some(holder), false) => format!("{} holder={holder}", fail.os),
            (Some(holder), true) => format!("holder={holder}"),
            (None, false) => fail.os.clone(),
            (None, true) => String::new(),
        };
        let os_ref = if os.is_empty() {
            None
        } else {
            Some(os.as_str())
        };
        log::write(&fail.code.log_line(os_ref));
        emit_event(
            events,
            EventMethod::Error,
            json!({
                "code": fail.code.wire_name(),
                "message": fail.code.user_message(),
            }),
        );
    }
    *last = Some(next);
}

struct GrabFail {
    code: ErrorCode,
    os: String,
    holder: Option<String>,
}

fn grab_fail(kind: DeviceIoKind, err: impl ToString, holder: Option<String>) -> GrabFail {
    let os = err.to_string();
    GrabFail {
        code: classify_device_io(kind, &os),
        os,
        holder,
    }
}

fn grab_nodes(nodes: &[PathBuf]) -> Result<Vec<evdev::Device>, GrabFail> {
    let mut devices = Vec::with_capacity(nodes.len());
    for path in nodes {
        match open_and_grab(path) {
            Ok(device) => devices.push(device),
            Err(err) => {
                let os = err.to_string();
                if classify_device_io(DeviceIoKind::EvdevGrab, &os) != ErrorCode::GrabConflict {
                    return Err(grab_fail(DeviceIoKind::EvdevOpen, os, None));
                }
                let holders = lookup_event_holders(path);
                match busy_decision(&holders) {
                    BusyDecision::Share => {
                        let device = evdev::Device::open(path).map_err(|open_err| {
                            grab_fail(DeviceIoKind::EvdevOpen, open_err, None)
                        })?;
                        let _ = device.set_nonblocking(true);
                        devices.push(device);
                    }
                    BusyDecision::Conflict { holder } => {
                        return Err(GrabFail {
                            code: ErrorCode::GrabConflict,
                            os,
                            holder,
                        });
                    }
                }
            }
        }
    }
    Ok(devices)
}

/// Takes the gamepad nodes from games while the profile remaps the stick, and
/// gives them back when it does not, so an unbound stick still works as a
/// gamepad. The daemon keeps reading them either way; the remap engine drops
/// stick input it has no binding for.
fn hold_gamepads(devices: &mut [evdev::Device], indexes: &[usize], hold: bool) {
    for &index in indexes {
        let Some(device) = devices.get_mut(index) else {
            continue;
        };
        let result = if hold { device.grab() } else { device.ungrab() };
        if let Err(err) = result {
            log::write(&ErrorCode::Io.log_line(Some(&err.to_string())));
        }
    }
}

fn open_and_grab(path: &PathBuf) -> std::io::Result<evdev::Device> {
    let mut device = evdev::Device::open(path)?;
    device.grab()?;
    let _ = device.set_nonblocking(true);
    Ok(device)
}

fn on_device_appeared(
    state: &Mutex<DaemonState<DaemonLighting>>,
    detected: &Detected,
    events: &broadcast::Sender<opentartarus_core::ipc::EventMsg>,
    epoch: &EngineEpoch,
) {
    let mut st = state.lock().expect("daemon state");
    let evdev_ok = detected.nodes.iter().all(|path| probe_evdev_readable(path));
    st.device_present = true;
    st.model = Some(detected.model);
    st.evdev_ok = evdev_ok;
    st.grab_conflict = None;
    st.engine = RemapEngine::new(detected.model);
    update_lighting_usb(&mut st.lighting, Some((detected.vid, detected.pid)));
    st.openrazer_available = st.lighting.available();
    let active = st.active_id.clone();
    if let Some(id) = active {
        let applied = handle_request(&mut st, Method::ApplyProfile, json!({ "id": id }), now_ms());
        commit_if_engine_replaced(applied.is_ok(), Some(Method::ApplyProfile), epoch);
        if applied.is_ok() {
            emit_event(events, EventMethod::ProfileApplied, json!({ "id": id }));
        }
    }
    emit_event(events, EventMethod::DeviceChanged, device_event_params(&st));
}

fn on_device_vanished(
    state: &Mutex<DaemonState<DaemonLighting>>,
    events: &broadcast::Sender<opentartarus_core::ipc::EventMsg>,
) {
    let mut st = state.lock().expect("daemon state");
    if let Some(opentartarus_core::record::RecordOutcome::Cancelled { reason }) =
        st.recorder.on_disconnect()
    {
        emit_event(
            events,
            EventMethod::RecordCancelled,
            json!({ "reason": record_reason_wire(reason) }),
        );
    }
    st.device_present = false;
    st.model = None;
    st.grab_conflict = None;
    update_lighting_usb(&mut st.lighting, None);
    st.openrazer_available = st.lighting.available();
    emit_event(events, EventMethod::DeviceChanged, device_event_params(&st));
}

fn record_reason_wire(reason: RecordReason) -> &'static str {
    match reason {
        RecordReason::Timeout => "timeout",
        RecordReason::User => "user",
        RecordReason::Disconnected => "disconnected",
    }
}

fn poll_record_timeout<L: LightingClient>(
    state: &Mutex<DaemonState<L>>,
    events: &broadcast::Sender<opentartarus_core::ipc::EventMsg>,
) {
    let mut st = state.lock().expect("daemon state");
    if let Some(opentartarus_core::record::RecordOutcome::Cancelled { reason }) =
        st.recorder.on_timeout(now_ms())
    {
        emit_event(
            events,
            EventMethod::RecordCancelled,
            json!({ "reason": record_reason_wire(reason) }),
        );
    }
}

fn handle_physical_event<L: LightingClient>(
    state: &Mutex<DaemonState<L>>,
    sink: &Mutex<Option<UinputSink>>,
    events: &broadcast::Sender<opentartarus_core::ipc::EventMsg>,
    epoch: &EngineEpoch,
    ev: RawEvent,
) {
    {
        let mut st = state.lock().expect("daemon state");
        if st.recorder.is_active() && ev.value == KEY_DOWN {
            if let Some(params) = record_params_from_event(&ev) {
                match handle_request(&mut st, Method::SubmitRecord, params, now_ms()) {
                    Ok(result) => {
                        commit_if_engine_replaced(true, Some(Method::SubmitRecord), epoch);
                        emit_event(events, EventMethod::Recorded, result);
                        if let Some(id) = st.active_id.as_deref() {
                            emit_event(events, EventMethod::ProfileApplied, json!({ "id": id }));
                        }
                    }
                    Err(err) => {
                        log::write(&err.log_line(None));
                    }
                }
                return;
            }
        }
    }
    let mut clock = SystemClock;
    match remap_physical_event(state, sink, epoch, ev, &mut clock) {
        Some(Ok(id)) => emit_event(events, EventMethod::ProfileApplied, json!({ "id": id })),
        Some(Err(err)) => log::write(&err.log_line(None)),
        None => {}
    }
}

fn record_params_from_event(ev: &RawEvent) -> Option<serde_json::Value> {
    if ev.ev_type == EV_REL && ev.code == REL_WHEEL {
        let scroll = if ev.value > 0 {
            ScrollDir::Up
        } else {
            ScrollDir::Down
        };
        return Some(json!({ "scroll": scroll }));
    }
    if ev.ev_type == EV_KEY {
        if let Some(button) = mouse_button_from_code(ev.code) {
            return Some(json!({ "button": button }));
        }
        if let Some(key) = token_from_evdev(ev.code) {
            return Some(json!({ "key": key, "modifiers": [] }));
        }
    }
    None
}

fn mouse_button_from_code(code: u16) -> Option<MouseButton> {
    [
        MouseButton::Left,
        MouseButton::Right,
        MouseButton::Middle,
        MouseButton::Back,
        MouseButton::Forward,
    ]
    .into_iter()
    .find(|&button| mouse_button_to_evdev(button) == code)
}

fn update_lighting_usb(lighting: &mut DaemonLighting, ids: Option<(u16, u16)>) {
    set_daemon_lighting_usb(lighting, ids);
}
