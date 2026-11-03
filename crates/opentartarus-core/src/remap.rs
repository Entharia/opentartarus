use crate::constants::{
    ANALOG_PRESS_RATIO, ANALOG_RELEASE_RATIO, STICK_FOUR_WAY_SWITCH_RATIO, TAP_HOLD_MS,
    USB_VID_RAZER,
};
use crate::keymap::{
    analog_axis_center, analog_axis_ratio_for, decode_hat, modifier_to_evdev,
    mouse_button_to_evdev, token_to_evdev, KeyMap, ABS_HAT0X, ABS_HAT0Y, ABS_RX, ABS_RY, ABS_X,
    ABS_Y, BTN_THUMBL, BTN_THUMBR, EV_ABS, EV_KEY, EV_REL, GAMEPAD_AXIS_MAX, REL_WHEEL,
};
use crate::types::{
    Action, DeviceModel, Edge, KeyId, MacroKind, MacroStep, MouseTarget, Profile, ProfileSwitch,
    ScrollDir, StickMode,
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
    stick_mode: StickMode,
    stick_rotation_deg: i32,
    /// The last position sent to the virtual controller in analog mode.
    pad_out: (i32, i32),
}

/// Turns a stick reading, `x` right and `y` down from -1 to 1, by `degrees`
/// clockwise as the player sees it. The result is clamped back into the
/// square the stick reports, since a turned corner reaches past it.
pub fn rotate_stick(x: f32, y: f32, degrees: i32) -> (f32, f32) {
    if degrees == 0 {
        return (x, y);
    }
    let (sin, cos) = (degrees as f32).to_radians().sin_cos();
    (
        (x * cos - y * sin).clamp(-1.0, 1.0),
        (x * sin + y * cos).clamp(-1.0, 1.0),
    )
}

/// The value a virtual controller axis gets for a stick ratio.
fn pad_axis_value(ratio: f32) -> i32 {
    (ratio * GAMEPAD_AXIS_MAX as f32).round() as i32
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
            stick_mode: StickMode::default(),
            stick_rotation_deg: 0,
            pad_out: (0, 0),
        }
    }

    pub fn apply_profile(&mut self, p: &Profile) {
        self.table = p.bindings_for_model(self.model);
        self.stick_mode = p.stick_mode;
    }

    /// Turns every later stick reading by `degrees`, clockwise.
    pub fn set_stick_rotation(&mut self, degrees: i32) {
        self.stick_rotation_deg = degrees;
    }

    pub fn stick_mode(&self) -> StickMode {
        self.stick_mode
    }

    /// Whether the active profile takes the thumbstick over: it binds part
    /// of it to keys, or hands it to games through the virtual controller.
    /// While it does neither, the daemon leaves the V2 Pro's gamepad node to
    /// games as it is.
    pub fn binds_stick(&self) -> bool {
        self.stick_mode.is_analog() || self.table.keys().any(|key| key.is_stick())
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
        if self.model == DeviceModel::V2Pro && ev.ev_type == EV_ABS {
            self.handle_v2_pro_stick(ev, sink, clock);
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

    /// Synapse stores a mode for the V2 Pro's stick in each onboard profile:
    /// left stick (X/Y), right stick (RX/RY) or D-pad (the hat). Whichever
    /// the active profile uses drives the same four stick directions, so a
    /// layout keeps working when the player switches profiles on the pad.
    /// A D-pad direction counts as the stick pushed all the way. Other axes
    /// (the triggers) are not on the pad and are ignored.
    fn handle_v2_pro_stick(
        &mut self,
        ev: RawEvent,
        sink: &mut impl EventSink,
        clock: &mut impl Clock,
    ) {
        let (code, value) = match ev.code {
            ABS_X | ABS_RX => (ABS_X, ev.value),
            ABS_Y | ABS_RY => (ABS_Y, ev.value),
            ABS_HAT0X => (ABS_X, ev.value.signum() * GAMEPAD_AXIS_MAX),
            ABS_HAT0Y => (ABS_Y, ev.value.signum() * GAMEPAD_AXIS_MAX),
            _ => return,
        };
        self.handle_analog(RawEvent { code, value, ..ev }, sink, clock);
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
        } else {
            self.analog_y = ev.value;
        }
        let (x, y) = rotate_stick(
            analog_axis_ratio_for(self.model, self.analog_x),
            analog_axis_ratio_for(self.model, self.analog_y),
            self.stick_rotation_deg,
        );
        match self.stick_mode {
            StickMode::Keys8 => {
                self.apply_analog_dir(KeyId::AnalogRight, x, ev, sink, clock);
                self.apply_analog_dir(KeyId::AnalogLeft, -x, ev, sink, clock);
                self.apply_analog_dir(KeyId::AnalogDown, y, ev, sink, clock);
                self.apply_analog_dir(KeyId::AnalogUp, -y, ev, sink, clock);
            }
            StickMode::Keys4 => self.apply_four_way(x, y, ev, sink, clock),
            StickMode::AnalogLeft | StickMode::AnalogRight => self.emit_pad_stick(x, y, sink),
        }
    }

    /// 4-way mode: hold only the direction pushed furthest. The held one
    /// stays until it falls below the release point or another direction is
    /// pushed clearly further.
    fn apply_four_way(
        &mut self,
        x: f32,
        y: f32,
        ev: RawEvent,
        sink: &mut impl EventSink,
        clock: &mut impl Clock,
    ) {
        let pushes = [
            (KeyId::AnalogRight, x),
            (KeyId::AnalogLeft, -x),
            (KeyId::AnalogDown, y),
            (KeyId::AnalogUp, -y),
        ];
        let (best, best_push) =
            pushes
                .into_iter()
                .fold((KeyId::AnalogUp, f32::MIN), |acc, item| {
                    if item.1 > acc.1 {
                        item
                    } else {
                        acc
                    }
                });
        let held = pushes.into_iter().find(|(id, _)| analog_pressed(self, *id));
        let keep = held.filter(|(_, push)| {
            *push > ANALOG_RELEASE_RATIO && push * STICK_FOUR_WAY_SWITCH_RATIO >= best_push
        });
        let next = keep
            .map(|(id, _)| id)
            .or_else(|| (best_push >= ANALOG_PRESS_RATIO).then_some(best));
        let held = held.map(|(id, _)| id);
        if next == held {
            return;
        }
        if let Some(id) = held {
            set_analog_pressed(self, id, false);
            self.dispatch(id, KEY_UP, ev, sink, clock);
        }
        if let Some(id) = next {
            set_analog_pressed(self, id, true);
            self.dispatch(id, KEY_DOWN, ev, sink, clock);
        }
    }

    /// Analog mode: the turned position goes to the virtual controller.
    fn emit_pad_stick(&mut self, x: f32, y: f32, sink: &mut impl EventSink) {
        let out = (pad_axis_value(x), pad_axis_value(y));
        if out == self.pad_out {
            return;
        }
        let (code_x, code_y) = if self.stick_mode == StickMode::AnalogRight {
            (ABS_RX, ABS_RY)
        } else {
            (ABS_X, ABS_Y)
        };
        for (code, value, last) in [
            (code_x, out.0, self.pad_out.0),
            (code_y, out.1, self.pad_out.1),
        ] {
            if value != last {
                sink.emit(Emitted {
                    keyboard: false,
                    ev_type: EV_ABS,
                    code,
                    value,
                });
            }
        }
        self.pad_out = out;
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
            // In analog mode an unbound stick click is the virtual
            // controller's stick button, so games get the whole stick.
            None if key_id == KeyId::StickClick && self.stick_mode.is_analog() => {
                let button = if self.stick_mode == StickMode::AnalogRight {
                    BTN_THUMBR
                } else {
                    BTN_THUMBL
                };
                if value == KEY_DOWN || value == KEY_UP {
                    sink.emit(Emitted {
                        keyboard: false,
                        ev_type: EV_KEY,
                        code: button,
                        value,
                    });
                }
            }
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
            stick_mode: StickMode::default(),
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
            stick_mode: StickMode::default(),
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
            stick_mode: StickMode::default(),
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
    fn v2_pro_stick_as_wasd_never_leaves_a_key_held_after_it_returns() {
        let p: Profile =
            serde_json::from_str(crate::pack::shipped_json("deadlock").unwrap()).unwrap();
        let mut e = RemapEngine::new(DeviceModel::V2Pro);
        e.apply_profile(&p);
        let mut s = FakeSink(vec![]);
        let mut c = FakeClock { t: 0 };
        let mut stick = |x: i32, y: i32, s: &mut FakeSink| {
            e.handle(v2_pro_event(3, ABS_X, x), s, &mut c);
            e.handle(v2_pro_event(3, ABS_Y, y), s, &mut c);
        };
        // Up, a diagonal, a flick straight across the centre, a slow circle,
        // then back to a resting wobble of a few hundred.
        let path = [
            (0, -32768),
            (23000, -23000),
            (32767, 0),
            (-32768, 0),
            (0, 32767),
            (-23000, 23000),
            (-32768, 0),
            (-23000, -23000),
            (0, -32768),
            (12000, -9000),
            (300, -250),
            (-120, 80),
        ];
        for (x, y) in path {
            stick(x, y, &mut s);
        }
        let mut held = std::collections::BTreeSet::new();
        for ev in &s.0 {
            match ev.value {
                1 => {
                    assert!(held.insert(ev.code), "{} pressed twice", ev.code);
                }
                0 => {
                    assert!(held.remove(&ev.code), "{} released unpressed", ev.code);
                }
                _ => {}
            }
        }
        assert!(held.is_empty(), "still held: {held:?}");
        let pressed: std::collections::BTreeSet<u16> =
            s.0.iter()
                .filter(|ev| ev.value == 1)
                .map(|ev| ev.code)
                .collect();
        let wasd: std::collections::BTreeSet<u16> =
            [KeyToken::W, KeyToken::A, KeyToken::S, KeyToken::D]
                .into_iter()
                .map(crate::keymap::token_to_evdev)
                .collect();
        assert_eq!(pressed, wasd, "the path pushes all four ways");
    }

    #[test]
    fn v2_pro_stick_drives_the_same_keys_in_every_synapse_mode() {
        let p: Profile =
            serde_json::from_str(crate::pack::shipped_json("deadlock").unwrap()).unwrap();
        let w = crate::keymap::token_to_evdev(KeyToken::W);
        let d = crate::keymap::token_to_evdev(KeyToken::D);
        let ctrl = crate::keymap::token_to_evdev(KeyToken::LeftCtrl);
        // (x code, y code, full push value, click button)
        let modes = [
            (ABS_X, ABS_Y, 32767, crate::keymap::BTN_THUMBL),
            (ABS_RX, ABS_RY, 32767, crate::keymap::BTN_THUMBR),
            (ABS_HAT0X, ABS_HAT0Y, 1, crate::keymap::BTN_THUMBR),
        ];
        for (x, y, full, click) in modes {
            let mut e = RemapEngine::new(DeviceModel::V2Pro);
            e.apply_profile(&p);
            let mut s = FakeSink(vec![]);
            let mut c = FakeClock { t: 0 };
            let codes = |s: &FakeSink| s.0.iter().map(|ev| (ev.code, ev.value)).collect::<Vec<_>>();
            e.handle(v2_pro_event(3, y, -full), &mut s, &mut c);
            e.handle(v2_pro_event(3, y, 0), &mut s, &mut c);
            assert_eq!(codes(&s), vec![(w, 1), (w, 0)], "up, mode {x}");
            s.0.clear();
            e.handle(v2_pro_event(3, x, full), &mut s, &mut c);
            e.handle(v2_pro_event(3, x, 0), &mut s, &mut c);
            assert_eq!(codes(&s), vec![(d, 1), (d, 0)], "right, mode {x}");
            s.0.clear();
            e.handle(v2_pro_event(1, click, 1), &mut s, &mut c);
            e.handle(v2_pro_event(1, click, 0), &mut s, &mut c);
            assert_eq!(codes(&s), vec![(ctrl, 1), (ctrl, 0)], "click, mode {x}");
        }
    }

    fn deadlock_on_v2_pro(mode: StickMode, rotation: i32) -> RemapEngine {
        let mut p: Profile =
            serde_json::from_str(crate::pack::shipped_json("deadlock").unwrap()).unwrap();
        p.stick_mode = mode;
        let mut e = RemapEngine::new(DeviceModel::V2Pro);
        e.apply_profile(&p);
        e.set_stick_rotation(rotation);
        e
    }

    fn push(e: &mut RemapEngine, x: i32, y: i32, s: &mut FakeSink) {
        let mut c = FakeClock { t: 0 };
        e.handle(v2_pro_event(3, ABS_X, x), s, &mut c);
        e.handle(v2_pro_event(3, ABS_Y, y), s, &mut c);
    }

    fn held_keys(s: &FakeSink) -> std::collections::BTreeSet<u16> {
        let mut held = std::collections::BTreeSet::new();
        for ev in s.0.iter().filter(|ev| ev.ev_type == EV_KEY) {
            if ev.value == 1 {
                held.insert(ev.code);
            } else if ev.value == 0 {
                held.remove(&ev.code);
            }
        }
        held
    }

    #[test]
    fn rotate_stick_turns_clockwise_and_stays_in_range() {
        let close = |(ax, ay): (f32, f32), (bx, by): (f32, f32)| {
            (ax - bx).abs() < 1e-4 && (ay - by).abs() < 1e-4
        };
        // Up (y negative) turned 90 degrees clockwise is right.
        assert!(close(rotate_stick(0.0, -1.0, 90), (1.0, 0.0)));
        // Right turned -90 (counter-clockwise) is up.
        assert!(close(rotate_stick(1.0, 0.0, -90), (0.0, -1.0)));
        assert!(close(rotate_stick(0.3, 0.4, 0), (0.3, 0.4)));
        assert!(close(rotate_stick(1.0, 0.0, 180), (-1.0, 0.0)));
        let (x, y) = rotate_stick(1.0, 1.0, 45);
        assert!(
            x.abs() <= 1.0 && y.abs() <= 1.0,
            "a turned corner is clamped"
        );
    }

    #[test]
    fn a_turned_stick_sends_the_keys_for_where_the_player_pushes() {
        let w = crate::keymap::token_to_evdev(KeyToken::W);
        // The player pushes forward but the stick reads right: -90 fixes it.
        let mut e = deadlock_on_v2_pro(StickMode::Keys8, -90);
        let mut s = FakeSink(vec![]);
        push(&mut e, 32767, 0, &mut s);
        assert_eq!(held_keys(&s), [w].into());
        push(&mut e, 0, 0, &mut s);
        assert!(held_keys(&s).is_empty());
    }

    #[test]
    fn eight_way_holds_two_keys_on_a_diagonal_and_four_way_holds_one() {
        let w = crate::keymap::token_to_evdev(KeyToken::W);
        let d = crate::keymap::token_to_evdev(KeyToken::D);
        let mut e = deadlock_on_v2_pro(StickMode::Keys8, 0);
        let mut s = FakeSink(vec![]);
        push(&mut e, 23000, -23000, &mut s);
        assert_eq!(held_keys(&s), [w, d].into());

        let mut e = deadlock_on_v2_pro(StickMode::Keys4, 0);
        let mut s = FakeSink(vec![]);
        // Mostly up, a little right: only W.
        push(&mut e, 12000, -30000, &mut s);
        assert_eq!(held_keys(&s), [w].into());
        // Swing to an exact diagonal: W stays, no flicker to D.
        push(&mut e, 23000, -23000, &mut s);
        assert_eq!(held_keys(&s), [w].into());
        // Clearly right now: W lets go, D presses.
        push(&mut e, 32000, -5000, &mut s);
        assert_eq!(held_keys(&s), [d].into());
        let presses = s.0.iter().filter(|ev| ev.value == 1).count();
        assert_eq!(presses, 2, "one press per direction, no flicker");
        push(&mut e, 0, 0, &mut s);
        assert!(held_keys(&s).is_empty());
    }

    #[test]
    fn analog_mode_hands_the_turned_stick_to_the_virtual_controller() {
        for (mode, code_x, code_y, button) in [
            (StickMode::AnalogLeft, ABS_X, ABS_Y, BTN_THUMBL),
            (StickMode::AnalogRight, ABS_RX, ABS_RY, BTN_THUMBR),
        ] {
            let mut e = deadlock_on_v2_pro(mode, -90);
            assert!(e.binds_stick(), "analog mode takes the stick over");
            let mut s = FakeSink(vec![]);
            push(&mut e, 32767, 0, &mut s);
            let axes: Vec<(u16, i32)> =
                s.0.iter()
                    .filter(|ev| ev.ev_type == EV_ABS)
                    .map(|ev| (ev.code, ev.value))
                    .collect();
            // Read right, turned -90: straight up on the controller.
            assert_eq!(axes.last(), Some(&(code_y, -32767)), "{mode:?}: {axes:?}");
            assert!(!axes.iter().any(|(c, v)| *c == code_x && v.abs() > 1));
            assert!(held_keys(&s).is_empty(), "no keys in analog mode");

            // The click is the stick button, because Deadlock's click binding
            // is replaced by an unbound one here.
            let mut p: Profile =
                serde_json::from_str(crate::pack::shipped_json("deadlock").unwrap()).unwrap();
            p.stick_mode = mode;
            p.model_bindings
                .get_mut(&DeviceModel::V2Pro)
                .unwrap()
                .remove(&KeyId::StickClick);
            e.apply_profile(&p);
            let mut s = FakeSink(vec![]);
            let mut c = FakeClock { t: 0 };
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
            assert_eq!(
                s.0.iter().map(|ev| (ev.code, ev.value)).collect::<Vec<_>>(),
                vec![(button, 1), (button, 0)]
            );
        }
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
