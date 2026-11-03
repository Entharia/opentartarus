use crate::handler::{switch_profile, DaemonState};
use crate::uinput_sink::UinputSink;
use opentartarus_core::error::ErrorCode;
use opentartarus_core::ipc::Method;
use opentartarus_core::lighting::LightingClient;
use opentartarus_core::remap::{Clock, Emitted, EventSink, RawEvent, RemapEngine};
use opentartarus_core::types::DeviceModel;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub struct EngineEpoch {
    value: AtomicU64,
}

impl EngineEpoch {
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    pub fn load(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    pub fn bump(&self) {
        self.value.fetch_add(1, Ordering::SeqCst);
    }
}

impl Default for EngineEpoch {
    fn default() -> Self {
        Self::new()
    }
}

pub fn method_replaces_engine(method: Method) -> bool {
    matches!(
        method,
        Method::ApplyProfile
            | Method::SetStickMode
            | Method::SetBinding
            | Method::ClearBinding
            | Method::RevertProfile
            | Method::SubmitRecord
    )
}

/// Bump the epoch only after a successful request that replaced `state.engine`.
/// Must be called while still holding the daemon-state mutex.
pub fn commit_if_engine_replaced(ok: bool, method: Option<Method>, epoch: &EngineEpoch) {
    if ok && method.is_some_and(method_replaces_engine) {
        epoch.bump();
    }
}

pub struct BriefLockSink<'a> {
    sink: &'a Mutex<Option<UinputSink>>,
}

impl<'a> BriefLockSink<'a> {
    pub fn new(sink: &'a Mutex<Option<UinputSink>>) -> Self {
        Self { sink }
    }
}

impl EventSink for BriefLockSink<'_> {
    fn emit(&mut self, e: Emitted) {
        if let Ok(mut held) = self.sink.lock() {
            if let Some(device) = held.as_mut() {
                device.emit(e);
            }
        }
    }
}

/// Plays one physical event through the engine. When the key was a
/// profile-switch key, carries the switch out and returns the id now live
/// (or the error the switch hit); `None` when the key asked for no switch.
pub fn remap_physical_event<L, C>(
    state: &Mutex<DaemonState<L>>,
    sink: &Mutex<Option<UinputSink>>,
    epoch: &EngineEpoch,
    ev: RawEvent,
    clock: &mut C,
) -> Option<Result<String, ErrorCode>>
where
    L: LightingClient,
    C: Clock,
{
    let (mut engine, epoch_at_take) = {
        let mut st = state.lock().expect("daemon state");
        let epoch_at_take = epoch.load();
        (take_engine(&mut st), epoch_at_take)
    };
    let mut emit = BriefLockSink::new(sink);
    engine.handle(ev, &mut emit, clock);
    let switch = engine.take_profile_switch();
    let mut st = state.lock().expect("daemon state");
    restore_engine_if_current(&mut st, engine, epoch_at_take, epoch);
    let switch = switch?;
    // A successful switch replaced the engine's table, so any taker still in
    // flight must not restore its stale copy over it.
    let outcome = switch_profile(&mut st, &switch);
    if outcome.is_ok() {
        epoch.bump();
    }
    Some(outcome)
}

pub fn tick_engine<L, C>(
    state: &Mutex<DaemonState<L>>,
    sink: &Mutex<Option<UinputSink>>,
    epoch: &EngineEpoch,
    clock: &mut C,
) where
    L: LightingClient,
    C: Clock,
{
    let (mut engine, epoch_at_take) = {
        let mut st = state.lock().expect("daemon state");
        if !st.engine.has_timed_work() {
            return;
        }
        let epoch_at_take = epoch.load();
        (take_engine(&mut st), epoch_at_take)
    };
    let mut emit = BriefLockSink::new(sink);
    engine.tick(&mut emit, clock);
    let mut st = state.lock().expect("daemon state");
    restore_engine_if_current(&mut st, engine, epoch_at_take, epoch);
}

fn take_engine<L: LightingClient>(state: &mut DaemonState<L>) -> RemapEngine {
    let model = state.model.unwrap_or(DeviceModel::V2);
    std::mem::replace(&mut state.engine, RemapEngine::new(model))
}

fn restore_engine_if_current<L: LightingClient>(
    state: &mut DaemonState<L>,
    engine: RemapEngine,
    epoch_at_take: u64,
    epoch: &EngineEpoch,
) {
    if epoch.load() == epoch_at_take {
        state.engine = engine;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{handle_request, DaemonState};
    use opentartarus_core::constants::{USB_PID_TARTARUS_V2, USB_VID_RAZER};
    use opentartarus_core::ipc::Method;
    use opentartarus_core::keymap::EV_KEY;
    use opentartarus_core::lighting::RecordingLighting;
    use opentartarus_core::paths::Paths;
    use opentartarus_core::record::Recorder;
    use opentartarus_core::remap::RemapEngine;
    use opentartarus_core::types::DeviceModel;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    const KP01_NATIVE_CODE: u16 = 2;
    const KEY_DOWN: i32 = 1;

    fn state() -> DaemonState<RecordingLighting> {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("opentartarus-pb-{n}"));
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
        }
    }

    struct PanicOnSleep;

    impl Clock for PanicOnSleep {
        fn now_ms(&self) -> u64 {
            0
        }
        fn sleep_ms(&mut self, _ms: u64) {
            panic!("macro handle must not sleep");
        }
    }

    #[test]
    fn macro_handle_does_not_sleep() {
        let mut st = state();
        bind_default_macro(&mut st);
        let state = Arc::new(Mutex::new(st));
        let sink = Arc::new(Mutex::new(None));
        let epoch = EngineEpoch::new();
        remap_physical_event(&state, &sink, &epoch, kp01_down(), &mut PanicOnSleep);
        assert!(state.try_lock().is_ok());
        assert!(sink.try_lock().is_ok());
        assert!(state.lock().expect("daemon state").engine.has_timed_work());
    }

    struct Collect(Vec<u16>);

    impl EventSink for Collect {
        fn emit(&mut self, e: Emitted) {
            self.0.push(e.code);
        }
    }

    struct NoSleep;

    impl Clock for NoSleep {
        fn now_ms(&self) -> u64 {
            0
        }
        fn sleep_ms(&mut self, _ms: u64) {}
    }

    const KP02_NATIVE_CODE: u16 = 3;

    fn key_down(code: u16) -> RawEvent {
        RawEvent {
            vid: USB_VID_RAZER,
            pid: USB_PID_TARTARUS_V2,
            ev_type: EV_KEY,
            code,
            value: KEY_DOWN,
        }
    }

    fn kp01_down() -> RawEvent {
        key_down(KP01_NATIVE_CODE)
    }

    fn probe_codes(engine: &mut RemapEngine, code: u16) -> Vec<u16> {
        let mut collect = Collect(Vec::new());
        engine.handle(key_down(code), &mut collect, &mut NoSleep);
        collect.0
    }

    struct ApplyDuringSleep {
        state: Arc<Mutex<DaemonState<RecordingLighting>>>,
        epoch: Arc<EngineEpoch>,
        apply_id: &'static str,
        method: Method,
        ok: bool,
        did: AtomicBool,
    }

    impl ApplyDuringSleep {
        fn apply_once(&self) {
            if self.did.swap(true, Ordering::SeqCst) {
                return;
            }
            let mut st = self.state.lock().expect("daemon state");
            let result = if self.ok {
                handle_request(
                    &mut st,
                    self.method,
                    match self.method {
                        Method::ApplyProfile => json!({ "id": self.apply_id }),
                        Method::SetLighting => json!({
                            "profile_id": "default",
                            "lighting": { "effect": "none", "brightness": 40 }
                        }),
                        _ => json!({}),
                    },
                    0,
                )
            } else {
                handle_request(&mut st, Method::ApplyProfile, json!({ "id": "missing" }), 0)
            };
            commit_if_engine_replaced(result.is_ok(), Some(self.method), &self.epoch);
        }
    }

    impl Clock for ApplyDuringSleep {
        fn now_ms(&self) -> u64 {
            self.apply_once();
            0
        }

        fn sleep_ms(&mut self, _ms: u64) {
            self.apply_once();
        }
    }

    fn bind_default_macro(st: &mut DaemonState<RecordingLighting>) {
        handle_request(st, Method::ApplyProfile, json!({ "id": "default" }), 0).unwrap();
        handle_request(
            st,
            Method::SetBinding,
            json!({
                "profile_id": "default",
                "key_id": "kp01",
                "action": {
                    "type": "macro",
                    "steps": [{
                        "kind": "key",
                        "key": "q",
                        "modifiers": [],
                        "edge": "down",
                        "delay_ms": 25
                    }]
                }
            }),
            0,
        )
        .unwrap();
    }

    #[test]
    fn apply_during_playback_keeps_applied_engine() {
        let mut st = state();
        bind_default_macro(&mut st);
        let state = Arc::new(Mutex::new(st));
        let sink = Arc::new(Mutex::new(None));
        let epoch = Arc::new(EngineEpoch::new());
        let mut clock = ApplyDuringSleep {
            state: Arc::clone(&state),
            epoch: Arc::clone(&epoch),
            apply_id: "dota-2",
            method: Method::ApplyProfile,
            ok: true,
            did: AtomicBool::new(false),
        };
        remap_physical_event(&state, &sink, &epoch, kp01_down(), &mut clock);
        let mut st = state.lock().expect("daemon state");
        assert_eq!(st.active_id.as_deref(), Some("dota-2"));
        let codes = probe_codes(&mut st.engine, KP02_NATIVE_CODE);
        assert_eq!(
            codes,
            vec![opentartarus_core::keymap::token_to_evdev(
                opentartarus_core::types::KeyToken::W
            )],
            "applied LoL engine must survive restore"
        );
        assert_eq!(epoch.load(), 1);
    }

    #[test]
    fn set_lighting_during_playback_does_not_drop_live_engine() {
        let mut st = state();
        bind_default_macro(&mut st);
        let state = Arc::new(Mutex::new(st));
        let sink = Arc::new(Mutex::new(None));
        let epoch = Arc::new(EngineEpoch::new());
        let mut clock = ApplyDuringSleep {
            state: Arc::clone(&state),
            epoch: Arc::clone(&epoch),
            apply_id: "default",
            method: Method::SetLighting,
            ok: true,
            did: AtomicBool::new(false),
        };
        remap_physical_event(&state, &sink, &epoch, kp01_down(), &mut clock);
        let mut st = state.lock().expect("daemon state");
        let codes = probe_codes(&mut st.engine, KP02_NATIVE_CODE);
        assert_eq!(
            codes,
            vec![KP02_NATIVE_CODE],
            "SetLighting must not bump; restore keeps the live engine"
        );
        assert_eq!(epoch.load(), 0);
    }

    #[test]
    fn failed_apply_during_playback_does_not_drop_live_engine() {
        let mut st = state();
        bind_default_macro(&mut st);
        let state = Arc::new(Mutex::new(st));
        let sink = Arc::new(Mutex::new(None));
        let epoch = Arc::new(EngineEpoch::new());
        let mut clock = ApplyDuringSleep {
            state: Arc::clone(&state),
            epoch: Arc::clone(&epoch),
            apply_id: "missing",
            method: Method::ApplyProfile,
            ok: false,
            did: AtomicBool::new(false),
        };
        remap_physical_event(&state, &sink, &epoch, kp01_down(), &mut clock);
        let mut st = state.lock().expect("daemon state");
        let codes = probe_codes(&mut st.engine, KP02_NATIVE_CODE);
        assert_eq!(
            codes,
            vec![KP02_NATIVE_CODE],
            "failed ApplyProfile must not bump; restore keeps the live engine"
        );
        assert_eq!(epoch.load(), 0);
        assert!(!method_replaces_engine(Method::SetLighting));
        assert!(method_replaces_engine(Method::ApplyProfile));
    }

    #[test]
    fn apply_winning_lock_before_take_keeps_applied_engine() {
        let mut st = state();
        bind_default_macro(&mut st);
        let state = Arc::new(Mutex::new(st));
        let sink = Arc::new(Mutex::new(None));
        let epoch = Arc::new(EngineEpoch::new());
        let mut held = state.lock().expect("daemon state");
        let worker_state = Arc::clone(&state);
        let worker_sink = Arc::clone(&sink);
        let worker_epoch = Arc::clone(&epoch);
        let worker = std::thread::spawn(move || {
            remap_physical_event(
                &worker_state,
                &worker_sink,
                &worker_epoch,
                kp01_down(),
                &mut NoSleep,
            );
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        handle_request(
            &mut held,
            Method::ApplyProfile,
            json!({ "id": "dota-2" }),
            0,
        )
        .unwrap();
        commit_if_engine_replaced(true, Some(Method::ApplyProfile), &epoch);
        drop(held);
        worker.join().expect("remap worker");
        let mut st = state.lock().expect("daemon state");
        assert_eq!(st.active_id.as_deref(), Some("dota-2"));
        let codes = probe_codes(&mut st.engine, KP02_NATIVE_CODE);
        assert_eq!(
            codes,
            vec![opentartarus_core::keymap::token_to_evdev(
                opentartarus_core::types::KeyToken::W
            )],
            "apply that wins the mutex before take must not be discarded on restore"
        );
        assert_eq!(epoch.load(), 1);
    }

    fn bind_kp01_switch(st: &mut DaemonState<RecordingLighting>, target: &str) {
        handle_request(st, Method::ApplyProfile, json!({ "id": "default" }), 0).unwrap();
        handle_request(
            st,
            Method::SetBinding,
            json!({
                "profile_id": "default",
                "key_id": "kp01",
                "action": { "type": "switch_profile", "profile": target }
            }),
            0,
        )
        .unwrap();
    }

    #[test]
    fn a_switch_key_makes_the_profile_live_and_bumps_the_epoch() {
        let mut st = state();
        bind_kp01_switch(&mut st, "dota-2");
        let state = Arc::new(Mutex::new(st));
        let sink = Arc::new(Mutex::new(None));
        let epoch = EngineEpoch::new();
        let applied = remap_physical_event(&state, &sink, &epoch, kp01_down(), &mut NoSleep);
        assert_eq!(applied, Some(Ok("dota-2".to_owned())));
        let mut st = state.lock().expect("daemon state");
        assert_eq!(st.active_id.as_deref(), Some("dota-2"));
        assert_eq!(
            epoch.load(),
            1,
            "the engine changed, so in-flight takers must not restore"
        );
        let codes = probe_codes(&mut st.engine, KP02_NATIVE_CODE);
        assert_eq!(
            codes,
            vec![opentartarus_core::keymap::token_to_evdev(
                opentartarus_core::types::KeyToken::W
            )],
            "the pad now plays the profile it switched to"
        );
    }

    #[test]
    fn a_switch_to_a_missing_profile_reports_it_and_keeps_the_live_engine() {
        let mut st = state();
        bind_kp01_switch(&mut st, "nope");
        let state = Arc::new(Mutex::new(st));
        let sink = Arc::new(Mutex::new(None));
        let epoch = EngineEpoch::new();
        let applied = remap_physical_event(&state, &sink, &epoch, kp01_down(), &mut NoSleep);
        assert_eq!(applied, Some(Err(ErrorCode::NotFound)));
        let st = state.lock().expect("daemon state");
        assert_eq!(st.active_id.as_deref(), Some("default"));
        assert_eq!(epoch.load(), 0, "nothing replaced the engine");
    }

    #[test]
    fn an_ordinary_key_asks_for_no_switch() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({ "id": "default" }), 0).unwrap();
        let state = Arc::new(Mutex::new(st));
        let sink = Arc::new(Mutex::new(None));
        let epoch = EngineEpoch::new();
        let applied = remap_physical_event(&state, &sink, &epoch, kp01_down(), &mut NoSleep);
        assert_eq!(applied, None);
        assert_eq!(epoch.load(), 0);
    }
}
