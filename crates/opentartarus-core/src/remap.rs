use crate::constants::{ANALOG_PRESS_RATIO, ANALOG_RELEASE_RATIO, TAP_HOLD_MS, USB_VID_RAZER};
use crate::keymap::{
    analog_axis_center, analog_axis_ratio_for, decode_hat, modifier_to_evdev,
    mouse_button_to_evdev, token_to_evdev, KeyMap, ABS_HAT0X, ABS_HAT0Y, ABS_X, ABS_Y, EV_ABS,
    EV_KEY, EV_REL, REL_WHEEL,
};
use crate::types::{
    Action, DeviceModel, Edge, KeyId, MacroKind, MacroStep, MouseTarget, Profile, ProfileSwitch,
    ScrollDir,
};
use std::collections::BTreeMap;

const KEY_DOWN: i32 = 1;
const KEY_REPEAT: i32 = 2;
const KEY_UP: i32 = 0;

#[derive(Debug, Clone)]
enum MacroOp {
    Emit { kind: MacroKind, value: i32 },
    Wait(u64),
}

#[derive(Debug, Clone)]
struct MacroPlay {
    ops: Vec<MacroOp>,
    index: usize,
    due_ms: u64,
}

pub fn allow_grab(vid: u16, pid: u16) -> bool {
    vid == USB_VID_RAZER && DeviceModel::from_usb_pid(pid).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawEvent {
    pub vid: u16,
    pub pid: u16,
    pub ev_type: u16,
    pub code: u16,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emitted {
    pub keyboard: bool,
    pub ev_type: u16,
    pub code: u16,
    pub value: i32,
}

pub trait EventSink {
    fn emit(&mut self, e: Emitted);
}

pub trait Clock {
    fn now_ms(&self) -> u64;
    fn sleep_ms(&mut self, ms: u64);
}

pub struct RemapEngine {
    model: DeviceModel,
    map: KeyMap,
    table: BTreeMap<KeyId, Action>,
    macro_busy: bool,
    macro_play: Option<MacroPlay>,
    hold: Option<(KeyId, u64, u32)>,
    profile_switch: Option<ProfileSwitch>,
    hat_x: i32,
    hat_y: i32,
    hat_dir: Option<KeyId>,
    analog_x: i32,
    analog_y: i32,
    analog_left: bool,
    analog_right: bool,
    analog_up: bool,
    analog_down: bool,
}

impl RemapEngine {
    pub fn new(model: DeviceModel) -> Self {
        Self {
            model,
            map: KeyMap::for_model(model),
            table: BTreeMap::new(),
            macro_busy: false,
            macro_play: None,
            hold: None,
            profile_switch: None,
            hat_x: 0,
            hat_y: 0,
            hat_dir: None,
            analog_x: analog_axis_center(model),
            analog_y: analog_axis_center(model),
            analog_left: false,
            analog_right: false,
            analog_up: false,
            analog_down: false,
        }
    }

    pub fn apply_profile(&mut self, p: &Profile) {
        self.table = p.bindings_for_model(self.model);
    }

    /// Whether the active profile binds any part of the thumbstick. While it
    /// does not, the daemon leaves the V2 Pro's gamepad node to games.
    pub fn binds_stick(&self) -> bool {
        self.table.keys().any(|key| key.is_stick())
    }

    pub fn has_timed_work(&self) -> bool {
        self.macro_play.is_some() || self.hold.is_some()
    }

    /// The profile change the last handled press asked for, if any. Taking
    /// it clears it, so one press asks exactly once.
    pub fn take_profile_switch(&mut self) -> Option<ProfileSwitch> {
        self.profile_switch.take()
    }

    pub fn pending_deadline_ms(&self) -> Option<u64> {
        if let Some(play) = &self.macro_play {
            return Some(play.due_ms);
        }
        if let Some((_, last_ms, rate_ms)) = self.hold {
            return Some(last_ms.saturating_add(u64::from(rate_ms)));
        }
        None
    }

    pub fn tick(&mut self, sink: &mut impl EventSink, clock: &mut impl Clock) {
        self.tick_macro(sink, clock);
        self.tick_hold(sink, clock);
    }

    fn tick_macro(&mut self, sink: &mut impl EventSink, clock: &mut impl Clock) {
        let now = clock.now_ms();
        let Some(play) = self.macro_play.as_mut() else {
            return;
        };
        while play.index < play.ops.len() {
            if now < play.due_ms {
                return;
            }
            match play.ops[play.index].clone() {
                MacroOp::Emit { kind, value } => {
                    play_kind_edge(&kind, value, sink);
                    play.index += 1;
                }
                MacroOp::Wait(ms) => {
                    play.due_ms = play.due_ms.saturating_add(ms);
                    play.index += 1;
                    if now < play.due_ms {
                        return;
                    }
                }
            }
        }
        if now < play.due_ms {
            return;
        }
        self.macro_play = None;
        self.macro_busy = false;
    }

    fn tick_hold(&mut self, sink: &mut impl EventSink, clock: &mut impl Clock) {
        let Some((key_id, last_ms, rate_ms)) = self.hold else {
            return;
        };
        let Some(Action::HoldRepeat { inner, .. }) = self.table.get(&key_id).cloned() else {
            return;
        };
        let rate = u64::from(rate_ms);
        if rate == 0 {
            return;
        }
        let now = clock.now_ms();
        let mut last = last_ms;
        while now.saturating_sub(last) >= rate {
            last = last.saturating_add(rate);
            play_action_edge(&inner, KEY_DOWN, sink);
        }
        if last != last_ms {
            self.hold = Some((key_id, last, rate_ms));
        }
    }

    pub fn handle(&mut self, ev: RawEvent, sink: &mut impl EventSink, clock: &mut impl Clock) {
        if !allow_grab(ev.vid, ev.pid) {
            return;
        }
        if ev.ev_type == EV_ABS && (ev.code == ABS_HAT0X || ev.code == ABS_HAT0Y) {
            self.handle_hat(ev, sink, clock);
            return;
        }
        if ev.ev_type == EV_ABS && (ev.code == ABS_X || ev.code == ABS_Y) {
            self.handle_analog(ev, sink, clock);
            return;
        }
        let qualifier = if ev.ev_type == EV_REL {
            ev.value
        } else {
            KEY_DOWN
        };
        let Some(key_id) = self.map.get(ev.ev_type, ev.code, qualifier) else {
            return;
        };
        if ev.ev_type == EV_REL {
            self.dispatch(key_id, KEY_DOWN, ev, sink, clock);
            if self.table.contains_key(&key_id)
                && !matches!(self.table.get(&key_id), Some(Action::Macro { .. }))
            {
                self.dispatch(key_id, KEY_UP, ev, sink, clock);
            }
            return;
        }
        self.dispatch(key_id, ev.value, ev, sink, clock);
    }

    fn handle_hat(&mut self, ev: RawEvent, sink: &mut impl EventSink, clock: &mut impl Clock) {
        if ev.code == ABS_HAT0X {
            self.hat_x = ev.value;
        } else {
            self.hat_y = ev.value;
        }
        let next = decode_hat(self.hat_x, self.hat_y);
        if next == self.hat_dir {
            return;
        }
        if let Some(prev) = self.hat_dir {
            self.dispatch(prev, KEY_UP, ev, sink, clock);
        }
        self.hat_dir = next;
        if let Some(id) = next {
            self.dispatch(id, KEY_DOWN, ev, sink, clock);
        }
    }

    fn handle_analog(&mut self, ev: RawEvent, sink: &mut impl EventSink, clock: &mut impl Clock) {
        if ev.code == ABS_X {
            self.analog_x = ev.value;
            let ratio = analog_axis_ratio_for(self.model, self.analog_x);
            self.apply_analog_dir(KeyId::AnalogRight, ratio, ev, sink, clock);
            self.apply_analog_dir(KeyId::AnalogLeft, -ratio, ev, sink, clock);
        } else {
            self.analog_y = ev.value;
            let ratio = analog_axis_ratio_for(self.model, self.analog_y);
            self.apply_analog_dir(KeyId::AnalogDown, ratio, ev, sink, clock);
            self.apply_analog_dir(KeyId::AnalogUp, -ratio, ev, sink, clock);
        }
    }

    fn apply_analog_dir(
        &mut self,
        id: KeyId,
        ratio: f32,
        ev: RawEvent,
        sink: &mut impl EventSink,
        clock: &mut impl Clock,
    ) {
        let pressed = analog_pressed(self, id);
        if !pressed && ratio >= ANALOG_PRESS_RATIO {
            set_analog_pressed(self, id, true);
            self.dispatch(id, KEY_DOWN, ev, sink, clock);
        } else if pressed && ratio <= ANALOG_RELEASE_RATIO {
            set_analog_pressed(self, id, false);
            self.dispatch(id, KEY_UP, ev, sink, clock);
        }
    }

    fn dispatch(
        &mut self,
        key_id: KeyId,
        value: i32,
        ev: RawEvent,
        sink: &mut impl EventSink,
        clock: &mut impl Clock,
    ) {
        match self.table.get(&key_id).cloned() {
            // The virtual devices have no axes and no gamepad buttons, so an
            // unbound stick, hat or gamepad button has nothing to pass on.
            None if ev.ev_type == EV_ABS || key_id.is_stick() => {}
            None => {
                sink.emit(Emitted {
                    keyboard: ev.ev_type == EV_KEY,
                    ev_type: ev.ev_type,
                    code: ev.code,
                    value: ev.value,
                });
            }
            Some(action) => match value {
                KEY_DOWN => self.play_press(key_id, &action, sink, clock),
                KEY_REPEAT => self.play_repeat(key_id, &action, sink, clock),
                KEY_UP => self.play_release(&action, sink),
                _ => {}
            },
        }
    }

    fn play_press(
        &mut self,
        key_id: KeyId,
        action: &Action,
        sink: &mut impl EventSink,
        clock: &mut impl Clock,
    ) {
        match action {
            Action::Macro { .. } if self.macro_busy => {}
            Action::Macro { steps } => {
                self.macro_busy = true;
                self.macro_play = Some(compile_macro(steps, clock.now_ms()));
                self.tick_macro(sink, clock);
            }
            Action::HoldRepeat { inner, rate_ms } => {
                play_action_edge(inner, KEY_DOWN, sink);
                self.hold = Some((key_id, clock.now_ms(), *rate_ms));
            }
            Action::SwitchProfile { profile } => {
                self.profile_switch = Some(ProfileSwitch::To(profile.clone()));
            }
            Action::NextProfile => self.profile_switch = Some(ProfileSwitch::Next),
            other => play_action_edge(other, KEY_DOWN, sink),
        }
    }

    fn play_repeat(
        &mut self,
        _key_id: KeyId,
        _action: &Action,
        _sink: &mut impl EventSink,
        _clock: &mut impl Clock,
    ) {
    }

    fn play_release(&mut self, action: &Action, sink: &mut impl EventSink) {
        match action {
            Action::Macro { .. } => {}
            Action::HoldRepeat { inner, .. } => {
                play_action_edge(inner, KEY_UP, sink);
                self.hold = None;
            }
            other => play_action_edge(other, KEY_UP, sink),
        }
    }
}

fn analog_pressed(engine: &RemapEngine, id: KeyId) -> bool {
    match id {
        KeyId::AnalogLeft => engine.analog_left,
        KeyId::AnalogRight => engine.analog_right,
        KeyId::AnalogUp => engine.analog_up,
        KeyId::AnalogDown => engine.analog_down,
        _ => false,
    }
}

fn set_analog_pressed(engine: &mut RemapEngine, id: KeyId, value: bool) {
    match id {
        KeyId::AnalogLeft => engine.analog_left = value,
        KeyId::AnalogRight => engine.analog_right = value,
        KeyId::AnalogUp => engine.analog_up = value,
        KeyId::AnalogDown => engine.analog_down = value,
        _ => {}
    }
}

fn compile_macro(steps: &[MacroStep], now_ms: u64) -> MacroPlay {
    let mut ops = Vec::new();
    for step in steps {
        match step.edge {
            Edge::Down => ops.push(MacroOp::Emit {
                kind: step.kind.clone(),
                value: KEY_DOWN,
            }),
            Edge::Up => ops.push(MacroOp::Emit {
                kind: step.kind.clone(),
                value: KEY_UP,
            }),
            Edge::Tap => {
                ops.push(MacroOp::Emit {
                    kind: step.kind.clone(),
                    value: KEY_DOWN,
                });
                ops.push(MacroOp::Wait(TAP_HOLD_MS));
                ops.push(MacroOp::Emit {
                    kind: step.kind.clone(),
                    value: KEY_UP,
                });
            }
        }
        if step.delay_ms > 0 {
            ops.push(MacroOp::Wait(u64::from(step.delay_ms)));
        }
    }
    MacroPlay {
        ops,
        index: 0,
        due_ms: now_ms,
    }
}

fn play_kind_edge(kind: &MacroKind, value: i32, sink: &mut impl EventSink) {
    match kind {
        MacroKind::Key { key, modifiers } => {
            emit_combo(sink, *key, modifiers, value);
        }
        MacroKind::Mouse { target } => emit_mouse(sink, target, value),
    }
}

fn play_action_edge(action: &Action, value: i32, sink: &mut impl EventSink) {
    match action {
        Action::Key { key, modifiers } => emit_combo(sink, *key, modifiers, value),
        Action::Mouse { target } => emit_mouse(sink, target, value),
        Action::Macro { .. }
        | Action::HoldRepeat { .. }
        | Action::SwitchProfile { .. }
        | Action::NextProfile => {}
    }
}

fn emit_combo(
    sink: &mut impl EventSink,
    key: crate::types::KeyToken,
    modifiers: &[crate::types::Modifier],
    value: i32,
) {
    let key_code = token_to_evdev(key);
    if value == KEY_DOWN {
        for modifier in modifiers {
            emit_key(sink, modifier_to_evdev(*modifier), KEY_DOWN);
        }
        emit_key(sink, key_code, KEY_DOWN);
    } else {
        emit_key(sink, key_code, KEY_UP);
        for modifier in modifiers.iter().rev() {
            emit_key(sink, modifier_to_evdev(*modifier), KEY_UP);
        }
    }
}

fn emit_mouse(sink: &mut impl EventSink, target: &MouseTarget, value: i32) {
    match target {
        MouseTarget::Button { button } => {
            sink.emit(Emitted {
                keyboard: false,
                ev_type: EV_KEY,
                code: mouse_button_to_evdev(*button),
                value,
            });
        }
        MouseTarget::Scroll { scroll } => {
            if value != KEY_DOWN {
                return;
            }
            let wheel = match scroll {
                ScrollDir::Up => 1,
                ScrollDir::Down => -1,
            };
            sink.emit(Emitted {
                keyboard: false,
                ev_type: EV_REL,
                code: REL_WHEEL,
                value: wheel,
            });
        }
    }
}

fn emit_key(sink: &mut impl EventSink, code: u16, value: i32) {
    sink.emit(Emitted {
        keyboard: true,
        ev_type: EV_KEY,
        code,
        value,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{USB_PID_NAGA_PRO_1, USB_PID_TARTARUS_V2, USB_VID_RAZER};
    use crate::types::{Action, DeviceModel, KeyId, KeyToken, Lighting, LightingEffect, Profile};
    use std::collections::BTreeMap;

    struct FakeSink(Vec<Emitted>);
    impl EventSink for FakeSink {
        fn emit(&mut self, e: Emitted) {
            self.0.push(e);
        }
    }
    struct FakeClock {
        t: u64,
    }
    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 {
            self.t
        }
        fn sleep_ms(&mut self, ms: u64) {
            self.t += ms;
        }
    }

    fn engine_with(kp01: Action) -> RemapEngine {
        let mut p = Profile {
            id: "t".into(),
            name: "t".into(),
            game: crate::types::GameId::Default,
            device_models: vec![DeviceModel::V2],
            bindings: BTreeMap::new(),
            model_bindings: BTreeMap::new(),
            lighting: Lighting {
                effect: LightingEffect::None,
                brightness: 80,
                color: None,
            },
            unknown_key_ids: false,
            setup_note: None,
            shipped_revision: None,
        };
        p.bindings.insert(KeyId::Kp01, kp01);
        let mut e = RemapEngine::new(DeviceModel::V2);
        e.apply_profile(&p);
        e
    }

    fn press_kp01() -> RawEvent {
        RawEvent {
            vid: USB_VID_RAZER,
            pid: USB_PID_TARTARUS_V2,
            ev_type: 1,
            code: 2,
            value: 1,
        }
    }

    #[test]
    fn kp01_emits_q() {
        let mut e = engine_with(Action::Key {
            key: KeyToken::Q,
            modifiers: vec![],
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        e.handle(
            RawEvent {
                value: 0,
                ..press_kp01()
            },
            &mut s,
            &mut c,
        );
        let q = crate::keymap::token_to_evdev(KeyToken::Q);
        assert!(s
            .0
            .iter()
            .any(|x| x.keyboard && x.code == q && x.value == 1));
        assert!(s
            .0
            .iter()
            .any(|x| x.keyboard && x.code == q && x.value == 0));
    }

    #[test]
    fn combo_ctrl_c() {
        let mut e = engine_with(Action::Key {
            key: KeyToken::C,
            modifiers: vec![crate::types::Modifier::Ctrl],
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        let codes: Vec<(u16, i32)> = s.0.iter().map(|x| (x.code, x.value)).collect();
        let ctrl = crate::keymap::modifier_to_evdev(crate::types::Modifier::Ctrl);
        let c_code = crate::keymap::token_to_evdev(KeyToken::C);
        assert_eq!(codes, vec![(ctrl, 1), (c_code, 1)]);
        s.0.clear();
        e.handle(
            RawEvent {
                value: 0,
                ..press_kp01()
            },
            &mut s,
            &mut c,
        );
        let up: Vec<(u16, i32)> = s.0.iter().map(|x| (x.code, x.value)).collect();
        assert_eq!(up, vec![(c_code, 0), (ctrl, 0)]);
    }

    #[test]
    fn macro_two_taps_respects_delay() {
        use crate::types::{Edge, MacroKind, MacroStep};
        let mut e = engine_with(Action::Macro {
            steps: vec![
                MacroStep {
                    kind: MacroKind::Key {
                        key: KeyToken::Num1,
                        modifiers: vec![],
                    },
                    edge: Edge::Tap,
                    delay_ms: 50,
                },
                MacroStep {
                    kind: MacroKind::Key {
                        key: KeyToken::Num2,
                        modifiers: vec![],
                    },
                    edge: Edge::Tap,
                    delay_ms: 0,
                },
            ],
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        while e.has_timed_work() {
            if let Some(due) = e.pending_deadline_ms() {
                if c.t < due {
                    c.t = due;
                }
            }
            e.tick(&mut s, &mut c);
        }
        assert!(c.t >= 50);
        let one = crate::keymap::token_to_evdev(KeyToken::Num1);
        let two = crate::keymap::token_to_evdev(KeyToken::Num2);
        let first_two =
            s.0.iter()
                .position(|x| x.code == two && x.value == 1)
                .unwrap();
        let first_one =
            s.0.iter()
                .position(|x| x.code == one && x.value == 1)
                .unwrap();
        assert!(first_one < first_two);
    }

    #[test]
    fn hold_repeat_and_passthrough_and_naga() {
        let mut e = engine_with(Action::HoldRepeat {
            inner: Box::new(Action::Key {
                key: KeyToken::E,
                modifiers: vec![],
            }),
            rate_ms: 40,
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        e.handle(
            RawEvent {
                value: 2,
                ..press_kp01()
            },
            &mut s,
            &mut c,
        );
        e.handle(
            RawEvent {
                value: 0,
                ..press_kp01()
            },
            &mut s,
            &mut c,
        );
        let ecode = crate::keymap::token_to_evdev(KeyToken::E);
        assert!(s.0.iter().any(|x| x.code == ecode && x.value == 1));
        assert!(s.0.iter().any(|x| x.code == ecode && x.value == 0));

        let mut s2 = FakeSink(vec![]);
        let kp02_down = RawEvent {
            vid: USB_VID_RAZER,
            pid: USB_PID_TARTARUS_V2,
            ev_type: 1,
            code: 3,
            value: 1,
        };
        e.handle(kp02_down, &mut s2, &mut c);
        e.handle(
            RawEvent {
                value: 0,
                ..kp02_down
            },
            &mut s2,
            &mut c,
        );
        assert!(s2.0.iter().any(|x| x.code == 3 && x.value == 1));
        assert!(s2.0.iter().any(|x| x.code == 3 && x.value == 0));

        assert!(!allow_grab(USB_VID_RAZER, USB_PID_NAGA_PRO_1));
        assert!(allow_grab(USB_VID_RAZER, USB_PID_TARTARUS_V2));
        assert!(allow_grab(
            USB_VID_RAZER,
            crate::constants::USB_PID_TARTARUS_PRO
        ));
    }

    #[test]
    fn naga_handle_emits_nothing() {
        let mut e = engine_with(Action::Key {
            key: KeyToken::Q,
            modifiers: vec![],
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(
            RawEvent {
                vid: USB_VID_RAZER,
                pid: USB_PID_NAGA_PRO_1,
                ev_type: 1,
                code: 2,
                value: 1,
            },
            &mut s,
            &mut c,
        );
        assert!(s.0.is_empty());
    }

    fn profile_with(model: DeviceModel, id: KeyId, action: Action) -> RemapEngine {
        let mut p = Profile {
            id: "t".into(),
            name: "t".into(),
            game: crate::types::GameId::Default,
            device_models: vec![model],
            bindings: BTreeMap::new(),
            model_bindings: BTreeMap::new(),
            lighting: Lighting {
                effect: LightingEffect::None,
                brightness: 80,
                color: None,
            },
            unknown_key_ids: false,
            setup_note: None,
            shipped_revision: None,
        };
        p.bindings.insert(id, action);
        let mut e = RemapEngine::new(model);
        e.apply_profile(&p);
        e
    }

    #[test]
    fn thumb_hat_eight_way_is_first_class() {
        let mut e = profile_with(
            DeviceModel::V2,
            KeyId::ThumbNe,
            Action::Key {
                key: KeyToken::W,
                modifiers: vec![],
            },
        );
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        let w = crate::keymap::token_to_evdev(KeyToken::W);
        e.handle(
            RawEvent {
                vid: USB_VID_RAZER,
                pid: USB_PID_TARTARUS_V2,
                ev_type: 3,
                code: 16,
                value: 1,
            },
            &mut s,
            &mut c,
        );
        e.handle(
            RawEvent {
                vid: USB_VID_RAZER,
                pid: USB_PID_TARTARUS_V2,
                ev_type: 3,
                code: 17,
                value: -1,
            },
            &mut s,
            &mut c,
        );
        assert!(s.0.iter().any(|x| x.code == w && x.value == 1));
        s.0.clear();
        e.handle(
            RawEvent {
                vid: USB_VID_RAZER,
                pid: USB_PID_TARTARUS_V2,
                ev_type: 3,
                code: 16,
                value: 0,
            },
            &mut s,
            &mut c,
        );
        e.handle(
            RawEvent {
                vid: USB_VID_RAZER,
                pid: USB_PID_TARTARUS_V2,
                ev_type: 3,
                code: 17,
                value: 0,
            },
            &mut s,
            &mut c,
        );
        assert!(s.0.iter().any(|x| x.code == w && x.value == 0));
    }

    #[test]
    fn analog_uses_press_and_release_ratios() {
        let mut e = profile_with(
            DeviceModel::Pro,
            KeyId::AnalogRight,
            Action::Key {
                key: KeyToken::D,
                modifiers: vec![],
            },
        );
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        let d = crate::keymap::token_to_evdev(KeyToken::D);
        let press = RawEvent {
            vid: USB_VID_RAZER,
            pid: crate::constants::USB_PID_TARTARUS_PRO,
            ev_type: 3,
            code: 0,
            value: 180,
        };
        e.handle(press, &mut s, &mut c);
        assert!(s.0.iter().any(|x| x.code == d && x.value == 1));
        s.0.clear();
        e.handle(
            RawEvent {
                value: 168,
                ..press
            },
            &mut s,
            &mut c,
        );
        assert!(s.0.is_empty());
        e.handle(
            RawEvent {
                value: 160,
                ..press
            },
            &mut s,
            &mut c,
        );
        assert!(s.0.iter().any(|x| x.code == d && x.value == 0));
    }

    #[test]
    fn mouse_action_emits_on_mouse_device() {
        use crate::types::MouseButton;
        let mut e = engine_with(Action::Mouse {
            target: crate::types::MouseTarget::Button {
                button: MouseButton::Left,
            },
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        e.handle(
            RawEvent {
                value: 0,
                ..press_kp01()
            },
            &mut s,
            &mut c,
        );
        assert!(s
            .0
            .iter()
            .any(|x| !x.keyboard && x.code == 272 && x.value == 1));
        assert!(s
            .0
            .iter()
            .any(|x| !x.keyboard && x.code == 272 && x.value == 0));
    }

    #[test]
    fn hold_repeat_emits_extra_down_when_rate_elapsed() {
        let mut e = engine_with(Action::HoldRepeat {
            inner: Box::new(Action::Key {
                key: KeyToken::E,
                modifiers: vec![],
            }),
            rate_ms: 40,
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        c.t = 40;
        e.tick(&mut s, &mut c);
        let ecode = crate::keymap::token_to_evdev(KeyToken::E);
        let downs =
            s.0.iter()
                .filter(|x| x.code == ecode && x.value == 1)
                .count();
        assert_eq!(downs, 2);
    }

    #[test]
    fn hold_repeat_ignores_kernel_auto_repeat() {
        let mut e = engine_with(Action::HoldRepeat {
            inner: Box::new(Action::Key {
                key: KeyToken::E,
                modifiers: vec![],
            }),
            rate_ms: 40,
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        c.t = 40;
        e.handle(
            RawEvent {
                value: 2,
                ..press_kp01()
            },
            &mut s,
            &mut c,
        );
        let ecode = crate::keymap::token_to_evdev(KeyToken::E);
        let downs =
            s.0.iter()
                .filter(|x| x.code == ecode && x.value == 1)
                .count();
        assert_eq!(downs, 1);
        e.tick(&mut s, &mut c);
        let downs =
            s.0.iter()
                .filter(|x| x.code == ecode && x.value == 1)
                .count();
        assert_eq!(downs, 2);
    }

    #[test]
    fn macro_press_does_not_sleep_and_other_keys_remap() {
        use crate::types::{Edge, MacroKind, MacroStep};
        let mut p = Profile {
            id: "t".into(),
            name: "t".into(),
            game: crate::types::GameId::Default,
            device_models: vec![DeviceModel::V2],
            bindings: BTreeMap::new(),
            model_bindings: BTreeMap::new(),
            lighting: Lighting {
                effect: LightingEffect::None,
                brightness: 80,
                color: None,
            },
            unknown_key_ids: false,
            setup_note: None,
            shipped_revision: None,
        };
        p.bindings.insert(
            KeyId::Kp01,
            Action::Macro {
                steps: vec![MacroStep {
                    kind: MacroKind::Key {
                        key: KeyToken::Num1,
                        modifiers: vec![],
                    },
                    edge: Edge::Tap,
                    delay_ms: 50,
                }],
            },
        );
        p.bindings.insert(
            KeyId::Kp02,
            Action::Key {
                key: KeyToken::Q,
                modifiers: vec![],
            },
        );
        let mut e = RemapEngine::new(DeviceModel::V2);
        e.apply_profile(&p);
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        assert_eq!(c.t, 0, "macro must not sleep inside handle");
        assert!(e.has_timed_work());
        let kp02 = RawEvent {
            vid: USB_VID_RAZER,
            pid: USB_PID_TARTARUS_V2,
            ev_type: 1,
            code: 3,
            value: 1,
        };
        e.handle(kp02, &mut s, &mut c);
        let q = crate::keymap::token_to_evdev(KeyToken::Q);
        assert!(s.0.iter().any(|x| x.code == q && x.value == 1));
        let one = crate::keymap::token_to_evdev(KeyToken::Num1);
        e.handle(press_kp01(), &mut s, &mut c);
        let ones = s.0.iter().filter(|x| x.code == one && x.value == 1).count();
        assert_eq!(ones, 1, "second macro press is ignored while busy");
    }

    #[test]
    fn a_switch_key_asks_for_the_profile_and_sends_nothing() {
        let mut e = engine_with(Action::SwitchProfile {
            profile: "dota-2".into(),
        });
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        assert_eq!(
            e.take_profile_switch(),
            Some(ProfileSwitch::To("dota-2".into()))
        );
        assert_eq!(e.take_profile_switch(), None, "one press asks once");
        e.handle(
            RawEvent {
                value: 0,
                ..press_kp01()
            },
            &mut s,
            &mut c,
        );
        assert!(s.0.is_empty(), "no keystroke may leak to the desktop");
        assert_eq!(e.take_profile_switch(), None, "the release asks nothing");
    }

    #[test]
    fn a_next_key_asks_for_the_next_profile() {
        let mut e = engine_with(Action::NextProfile);
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(press_kp01(), &mut s, &mut c);
        assert_eq!(e.take_profile_switch(), Some(ProfileSwitch::Next));
        assert!(s.0.is_empty());
    }

    fn v2_pro_event(ev_type: u16, code: u16, value: i32) -> RawEvent {
        RawEvent {
            vid: USB_VID_RAZER,
            pid: crate::constants::USB_PID_TARTARUS_V2_PRO,
            ev_type,
            code,
            value,
        }
    }

    #[test]
    fn v2_pro_is_grabbed_and_its_extra_keys_remap() {
        assert!(allow_grab(
            USB_VID_RAZER,
            crate::constants::USB_PID_TARTARUS_V2_PRO
        ));
        let mut e = profile_with(
            DeviceModel::V2Pro,
            KeyId::Kp21,
            Action::Key {
                key: KeyToken::F1,
                modifiers: vec![],
            },
        );
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        let grave = 41;
        e.handle(v2_pro_event(1, grave, 1), &mut s, &mut c);
        e.handle(v2_pro_event(1, grave, 0), &mut s, &mut c);
        let f1 = crate::keymap::token_to_evdev(KeyToken::F1);
        assert_eq!(
            s.0.iter().map(|x| (x.code, x.value)).collect::<Vec<_>>(),
            vec![(f1, 1), (f1, 0)]
        );
    }

    #[test]
    fn v2_pro_gamepad_stick_presses_past_the_threshold() {
        let mut e = profile_with(
            DeviceModel::V2Pro,
            KeyId::AnalogUp,
            Action::Key {
                key: KeyToken::W,
                modifiers: vec![],
            },
        );
        assert!(e.binds_stick());
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        let w = crate::keymap::token_to_evdev(KeyToken::W);
        // Resting and small wobble around 0 do nothing.
        e.handle(v2_pro_event(3, ABS_Y, -771), &mut s, &mut c);
        e.handle(v2_pro_event(3, ABS_X, 2162), &mut s, &mut c);
        assert!(s.0.is_empty(), "{:?}", s.0);
        e.handle(v2_pro_event(3, ABS_Y, -30000), &mut s, &mut c);
        assert_eq!(s.0.last().map(|x| (x.code, x.value)), Some((w, 1)));
        e.handle(v2_pro_event(3, ABS_Y, -1000), &mut s, &mut c);
        assert_eq!(s.0.last().map(|x| (x.code, x.value)), Some((w, 0)));
    }

    #[test]
    fn an_unbound_stick_and_click_pass_nothing_to_the_virtual_devices() {
        let mut e = profile_with(
            DeviceModel::V2Pro,
            KeyId::Kp01,
            Action::Key {
                key: KeyToken::Q,
                modifiers: vec![],
            },
        );
        assert!(!e.binds_stick());
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        e.handle(v2_pro_event(3, ABS_X, 32767), &mut s, &mut c);
        e.handle(v2_pro_event(3, ABS_X, 0), &mut s, &mut c);
        e.handle(
            v2_pro_event(1, crate::keymap::BTN_THUMBL, 1),
            &mut s,
            &mut c,
        );
        e.handle(
            v2_pro_event(1, crate::keymap::BTN_THUMBL, 0),
            &mut s,
            &mut c,
        );
        assert!(s.0.is_empty(), "{:?}", s.0);
    }

    #[test]
    fn a_v2_profile_drives_the_same_keys_on_the_v2_pro() {
        // The shipped Default binds kp06 to Tab; on the V2 Pro, kp06 is the
        // key that sends Tab, so the profile lands where it belongs.
        let p: Profile =
            serde_json::from_str(crate::pack::shipped_json("default").unwrap()).unwrap();
        let mut e = RemapEngine::new(DeviceModel::V2Pro);
        e.apply_profile(&p);
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        let tab = crate::keymap::token_to_evdev(KeyToken::Tab);
        e.handle(v2_pro_event(1, tab, 1), &mut s, &mut c);
        assert_eq!(s.0.last().map(|x| (x.code, x.value)), Some((tab, 1)));
    }
}
