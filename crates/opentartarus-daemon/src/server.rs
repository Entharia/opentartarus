use crate::handler::{handle_request, DaemonState};
use crate::playback::{commit_if_engine_replaced, EngineEpoch};
use opentartarus_core::codec::{decode_frame, encode_frame};
use opentartarus_core::constants::IPC_MAX_MESSAGE_BYTES;
use opentartarus_core::error::ErrorCode;
use opentartarus_core::ipc::{
    parse_request, request_error_message, EventMethod, EventMsg, EventTag, Method, ResTag,
    ResponseMsg, WireError,
};
use opentartarus_core::lighting::LightingClient;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

const SOCKET_DIR_MODE: u32 = 0o700;
const READ_CHUNK: usize = 4096;

pub fn bind_exclusive(socket: &Path) -> io::Result<UnixListener> {
    if let Some(parent) = socket.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(SOCKET_DIR_MODE))?;
    }
    if socket.exists() {
        match std::os::unix::net::UnixStream::connect(socket) {
            Ok(_) => {
                return Err(io::Error::new(ErrorKind::AddrInUse, "already_running"));
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => return Err(err),
            Err(_) => {
                fs::remove_file(socket)?;
            }
        }
    }
    UnixListener::bind(socket)
}

pub fn listen_error_code(err: &io::Error) -> ErrorCode {
    if err.kind() == ErrorKind::AddrInUse {
        ErrorCode::AlreadyRunning
    } else {
        ErrorCode::Io
    }
}

pub fn emit_event(tx: &broadcast::Sender<EventMsg>, method: EventMethod, params: Value) {
    let _ = tx.send(EventMsg {
        r#type: EventTag,
        method,
        params,
    });
}

pub async fn accept_loop<L: LightingClient + Send + 'static>(
    listener: UnixListener,
    state: Arc<Mutex<DaemonState<L>>>,
    events: broadcast::Sender<EventMsg>,
    quit: Arc<AtomicBool>,
    clients: Arc<AtomicUsize>,
    epoch: Arc<EngineEpoch>,
) {
    loop {
        if quit.load(Ordering::SeqCst) {
            break;
        }
        match listener.accept().await {
            Ok((stream, _)) => {
                clients.fetch_add(1, Ordering::SeqCst);
                let state = Arc::clone(&state);
                let events = events.clone();
                let quit = Arc::clone(&quit);
                let clients = Arc::clone(&clients);
                let epoch = Arc::clone(&epoch);
                tokio::spawn(async move {
                    let _guard = ClientGuard(clients);
                    handle_client(stream, state, events, quit, epoch).await;
                });
            }
            Err(_) => {
                if quit.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
}

struct ClientGuard(Arc<AtomicUsize>);

impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn handle_client<L: LightingClient + Send>(
    stream: UnixStream,
    state: Arc<Mutex<DaemonState<L>>>,
    events: broadcast::Sender<EventMsg>,
    quit: Arc<AtomicBool>,
    epoch: Arc<EngineEpoch>,
) {
    let (mut reader, mut writer) = stream.into_split();
    let mut events_rx = events.subscribe();
    let mut buf = Vec::new();
    loop {
        if quit.load(Ordering::SeqCst) {
            break;
        }
        tokio::select! {
            frame = read_frame(&mut reader, &mut buf) => {
                match frame {
                    Ok(payload) => {
                        let method = request_method(&payload);
                        let out = {
                            let mut st = state.lock().expect("daemon state");
                            let out = dispatch_json(&mut *st, &payload);
                            commit_if_engine_replaced(response_ok(&out), method, &epoch);
                            out
                        };
                        if write_bytes(&mut writer, &out).await.is_err() {
                            break;
                        }
                        after_request(method, &out, &state, &events, &quit);
                    }
                    Err(_) => break,
                }
            }
            event = events_rx.recv() => {
                match event {
                    Ok(msg) => {
                        if write_json(&mut writer, &msg).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

fn request_method(bytes: &[u8]) -> Option<Method> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    parse_request(&value).ok().map(|req| req.method)
}

fn response_ok(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| value.get("ok").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn after_request<L: LightingClient>(
    method: Option<Method>,
    response: &[u8],
    state: &Mutex<DaemonState<L>>,
    events: &broadcast::Sender<EventMsg>,
    quit: &Arc<AtomicBool>,
) {
    let Ok(value) = serde_json::from_slice::<Value>(response) else {
        return;
    };
    let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false);
    match method {
        Some(Method::QuitDaemon) => {
            emit_event(events, EventMethod::DaemonStopping, json!({}));
            quit.store(true, Ordering::SeqCst);
        }
        Some(Method::ApplyProfile) if ok => {
            if let Some(id) = value.pointer("/result/id").and_then(Value::as_str) {
                emit_event(events, EventMethod::ProfileApplied, json!({ "id": id }));
            }
        }
        Some(Method::SubmitRecord) if ok => {
            if let Some(result) = value.get("result") {
                emit_event(events, EventMethod::Recorded, result.clone());
                if let Ok(st) = state.lock() {
                    if let Some(id) = st.active_id.as_deref() {
                        emit_event(events, EventMethod::ProfileApplied, json!({ "id": id }));
                    }
                }
            }
        }
        Some(Method::StopRecord)
            if ok
                && value
                    .pointer("/result/cancelled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false) =>
        {
            emit_event(
                events,
                EventMethod::RecordCancelled,
                json!({ "reason": "user" }),
            );
        }
        Some(
            Method::SetBinding
            | Method::ClearBinding
            | Method::SetLighting
            | Method::RevertProfile
            | Method::SetStickMode,
        ) if ok => {
            if let Some(id) = value.pointer("/result/id").and_then(Value::as_str) {
                emit_event(events, EventMethod::ProfileApplied, json!({ "id": id }));
            }
        }
        _ => {}
    }
}

async fn read_frame(reader: &mut OwnedReadHalf, buf: &mut Vec<u8>) -> Result<Vec<u8>, ErrorCode> {
    loop {
        if buf.len() >= 4 {
            let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            if len == 0 || len > IPC_MAX_MESSAGE_BYTES {
                return Err(ErrorCode::InvalidProfile);
            }
            if buf.len() >= 4 + len as usize {
                let (payload, consumed) = decode_frame(buf)?;
                buf.drain(..consumed);
                return Ok(payload);
            }
        }
        let mut chunk = [0u8; READ_CHUNK];
        let n = reader.read(&mut chunk).await.map_err(|_| ErrorCode::Io)?;
        if n == 0 {
            return Err(ErrorCode::Io);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

async fn write_bytes(writer: &mut OwnedWriteHalf, payload: &[u8]) -> Result<(), ErrorCode> {
    let frame = encode_frame(payload)?;
    writer.write_all(&frame).await.map_err(|_| ErrorCode::Io)
}

async fn write_json<T: serde::Serialize>(
    writer: &mut OwnedWriteHalf,
    value: &T,
) -> Result<(), ErrorCode> {
    let payload = serde_json::to_vec(value).map_err(|_| ErrorCode::InvalidProfile)?;
    write_bytes(writer, &payload).await
}

pub fn dispatch_json<L: LightingClient>(state: &mut DaemonState<L>, bytes: &[u8]) -> Vec<u8> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return encode_error(generated_id(), ErrorCode::InvalidProfile),
    };
    match parse_request(&value) {
        Ok(req) => {
            let result = handle_request(state, req.method, req.params, now_ms);
            encode_response(req.id, result, Some(req.method))
        }
        Err(err) => {
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(generated_id);
            encode_wire_error(id, err)
        }
    }
}

fn generated_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn encode_error(id: String, code: ErrorCode) -> Vec<u8> {
    encode_response(id, Err(code), None)
}

fn encode_wire_error(id: String, error: WireError) -> Vec<u8> {
    let msg = ResponseMsg {
        r#type: ResTag,
        id,
        ok: false,
        result: None,
        error: Some(error),
    };
    serde_json::to_vec(&msg).expect("ResponseMsg is serializable")
}

fn encode_response(
    id: String,
    result: Result<Value, ErrorCode>,
    method: Option<Method>,
) -> Vec<u8> {
    let msg = match result {
        Ok(value) => ResponseMsg {
            r#type: ResTag,
            id,
            ok: true,
            result: Some(value),
            error: None,
        },
        Err(code) => ResponseMsg {
            r#type: ResTag,
            id,
            ok: false,
            result: None,
            error: Some(WireError {
                code,
                message: request_error_message(code, method).to_string(),
            }),
        },
    };
    serde_json::to_vec(&msg).expect("ResponseMsg is serializable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::DaemonState;
    use opentartarus_core::lighting::RecordingLighting;
    use opentartarus_core::paths::Paths;
    use opentartarus_core::record::Recorder;
    use opentartarus_core::remap::RemapEngine;
    use opentartarus_core::types::{DeviceModel, Layer};
    use serde_json::Value;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn state() -> DaemonState<RecordingLighting> {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("opentartarus-s-{n}"));
        let paths = Paths::from_dirs(root.join("c"), root.join("r"), root.join("s"));
        DaemonState {
            paths,
            engine: RemapEngine::new(DeviceModel::V2),
            recorder: Recorder::default(),
            lighting: RecordingLighting {
                available: true,
                last: None,
            },
            active_id: None,
            device_present: true,
            model: Some(DeviceModel::V2),
            openrazer_available: true,
            evdev_ok: true,
            uinput_ok: true,
            grab_conflict: None,
            stick_rotation_deg: 0,
            record_layer: Layer::Base,
        }
    }

    #[test]
    fn dispatch_json_get_status_ok() {
        let mut st = state();
        let bytes = br#"{"type":"req","id":"u1","method":"GetStatus","params":{}}"#;
        let out = dispatch_json(&mut st, bytes);
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["type"], "res");
        assert_eq!(v["id"], "u1");
        assert_eq!(v["ok"], true);
        assert_eq!(v["result"]["device"]["pid"], "022b");
    }

    #[test]
    fn dispatch_json_unknown_method_is_not_found() {
        let mut st = state();
        let bytes = br#"{"type":"req","id":"u2","method":"Explode","params":{}}"#;
        let out = dispatch_json(&mut st, bytes);
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "not_found");
        assert_eq!(v["error"]["message"], "Unknown method");
    }

    #[test]
    fn dispatch_json_submit_record_without_session_is_not_recording() {
        let mut st = state();
        let bytes = br#"{"type":"req","id":"u3","method":"SubmitRecord","params":{"key":"a","modifiers":[]}}"#;
        let out = dispatch_json(&mut st, bytes);
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "not_found");
        assert_eq!(v["error"]["message"], "Not recording.");
    }

    #[tokio::test]
    async fn stale_socket_is_unlinked_then_bound() {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("opentartarus-sock-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("daemon.sock");
        let stale = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        drop(stale);
        assert!(socket.exists());
        let listener = bind_exclusive(&socket).expect("dead leftover socket must be replaced");
        drop(listener);
        let _ = std::fs::remove_file(&socket);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn live_socket_stays_already_running() {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("opentartarus-live-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("daemon.sock");
        let live = bind_exclusive(&socket).unwrap();
        let err = bind_exclusive(&socket).expect_err("live listener must win");
        assert_eq!(err.kind(), ErrorKind::AddrInUse);
        assert_eq!(listen_error_code(&err), ErrorCode::AlreadyRunning);
        assert_ne!(listen_error_code(&err), ErrorCode::GrabConflict);
        assert_ne!(listen_error_code(&err), ErrorCode::NotFound);
        drop(live);
        let _ = std::fs::remove_file(&socket);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn second_daemon_is_already_running_not_grab_or_missing() {
        let err = std::io::Error::new(ErrorKind::AddrInUse, "already_running");
        assert_eq!(listen_error_code(&err), ErrorCode::AlreadyRunning);
        assert_ne!(listen_error_code(&err), ErrorCode::GrabConflict);
        assert_ne!(listen_error_code(&err), ErrorCode::NotFound);
        assert_ne!(listen_error_code(&err), ErrorCode::Permission);
    }
}
