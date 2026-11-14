use crate::client::{self, FixPermissionsOutcome, Outgoing};
use crate::keys::{
    capture_window_key, is_combo_id, is_new_profile_id, query_focused_id, submit_record_key_params,
    submit_record_mouse_params, KeyCapture, NEW_PROFILE_INPUT_ID,
};
use crate::theme;
use iced::advanced::widget::Id;
use iced::futures::channel::mpsc;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::mouse;
use iced::widget::text_input;
use iced::window::{self, Mode};
use iced::{event, Event, Subscription, Task, Theme};
use opentartarus_core::constants::{
    HOLD_REPEAT_RATE_DEFAULT_MS, MACRO_MAX_DELAY_MS, MACRO_MAX_STEPS, RECORD_TIMEOUT_MS,
    STICK_ROTATION_MAX_DEG,
};
use opentartarus_core::error::ErrorCode;
use opentartarus_core::ipc::{EventMethod, Method, WireError};
use opentartarus_core::light_presets::{preset_by_id, PresetGroup};
use opentartarus_core::lighting::{fill_row, solid_frame, FRAME_ROWS, HOME_ROW};
use opentartarus_core::pack;
use opentartarus_core::paths::Paths;
use opentartarus_core::store;
use opentartarus_core::types::{
    Action, DeviceModel, Edge, KeyId, KeyToken, Layer, Lighting, LightingEffect, MacroKind,
    MacroStep, Modifier, MouseTarget, Profile, StickMode,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Connecting,
    Running,
    FailedStart,
}

/// Which panel the centre column shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Keys,
    Lighting,
}

#[derive(Debug, Clone)]
pub struct ProfileRow {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub can_revert: bool,
    /// True for a profile the user made. Shipped profiles can be reverted
    /// but never deleted; custom ones are the reverse.
    pub can_delete: bool,
    /// The profile's lighting colour, resolved once when the list refreshes so
    /// the sidebar never reads a file while drawing.
    pub color: Option<[u8; 3]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Banner {
    Starting,
    CouldNotStart(String),
    NoDevice,
    Permission,
    Disconnect,
    GrabConflict { name: Option<String> },
    UnplugAfterFix,
    SignOut,
    Other(String),
}

#[derive(Debug, Clone)]
pub enum Message {
    IpcReady(mpsc::UnboundedSender<Outgoing>),
    DaemonStarting,
    DaemonFailed(String),
    DaemonConnected,
    DaemonStopping,
    IpcResponse {
        method: Option<Method>,
        ok: bool,
        result: Option<Value>,
        error: Option<WireError>,
    },
    IpcEvent {
        method: EventMethod,
        params: Value,
    },
    SelectProfile(String),
    SelectKey(KeyId),
    Record,
    StopRecord,
    ClearBinding,
    ComboChanged(String),
    RefreshComboFocus,
    ComboFocusChanged(Option<Id>),
    ComboKey {
        key: Key,
        modifiers: keyboard::Modifiers,
    },
    ComboKeyResolved {
        key: Key,
        modifiers: keyboard::Modifiers,
        focused: Option<Id>,
    },
    HoldRepeat(bool),
    MousePick(MouseTarget),
    /// Binds the selected key to one key that the combo box cannot type,
    /// such as a bare Shift.
    KeyPick(KeyToken),
    AddMacroStep,
    DeleteMacroStep(usize),
    MacroDelay(usize, String),
    LightingEffect(EffectChoice),
    LightingBrightness(u8),
    LightingColor(usize, u8),
    /// Sets how the active profile reads the thumbstick.
    StickModePick(StickChoice),
    /// Moves the rotation slider; sent to the daemon on release.
    StickRotationChanged(i32),
    StickRotationReleased,
    RevertProfile,
    FixPermissions,
    FixPermissionsDone(FixPermissionsOutcome),
    Quit,
    CloseRequested(window::Id),
    WindowId(Option<window::Id>),
    SelectTab(Tab),
    ToggleMenu,
    HoverKey(Option<KeyId>),
    LightingPreset([u8; 3]),
    /// Picks which row of a custom frame the colour controls edit.
    LightingRow(usize),
    /// Replaces the custom frame with a preset's, by preset id.
    LightFramePreset(&'static str),
    /// Shows one group of presets in the lighting tab's side panel.
    PresetGroupPick(PresetGroup),
    DragWindow,
    MinimizeWindow,
    ToggleMaximize,
    CloseWindow,
    /// Opens the inline name box at the bottom of the profile list.
    StartNewProfile,
    NewProfileNameChanged(String),
    /// Creates the profile from the typed name, as a copy of the selected one.
    SubmitNewProfile,
    CancelNewProfile,
    DeleteProfile(String),
    /// Binds the selected key to step to the next profile.
    NextProfilePick,
    /// Binds the selected key to jump to one profile.
    SwitchProfilePick(ProfileChoice),
    /// Shows and edits the normal bindings or the Hypershift layer.
    SelectLayer(Layer),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    HideToTray,
    ExitUi,
}

pub fn close_action(wayland: bool) -> CloseAction {
    if wayland {
        CloseAction::ExitUi
    } else {
        CloseAction::HideToTray
    }
}

pub fn session_is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Whether the window may round its own corners.
///
/// Rounding a frameless window needs a transparent surface, so the corner
/// pixels can be left clear. Wayland grants that; X11 and XWayland drop the
/// connection instead and the process dies at startup. Measured on this
/// machine: identical build, `transparent: true` exits under XWayland,
/// `transparent: false` runs. So the corners are square on X11 rather than
/// the app being unusable there.
pub fn window_is_rounded() -> bool {
    static ROUNDED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ROUNDED.get_or_init(session_is_wayland)
}

/// The window pane's corner radius, and the matching radius the header and
/// status bar use on their outer corners. Zero where rounding is unavailable.
pub fn window_radius() -> f32 {
    if window_is_rounded() {
        theme::RADIUS_WINDOW
    } else {
        theme::BORDER_NONE
    }
}

pub struct App {
    pub ipc_tx: Option<mpsc::UnboundedSender<Outgoing>>,
    pub phase: Phase,
    pub profiles: Vec<ProfileRow>,
    pub active_profile_id: Option<String>,
    pub selected_profile_id: Option<String>,
    pub device_present: bool,
    pub ever_present: bool,
    pub model: Option<DeviceModel>,
    pub openrazer: bool,
    pub evdev_ok: bool,
    pub uinput_ok: bool,
    pub grab_conflict: Option<String>,
    pub bindings: BTreeMap<KeyId, Action>,
    /// The active profile's Hypershift layer (V2 Pro only).
    pub hypershift_bindings: BTreeMap<KeyId, Action>,
    /// The layer the keypad shows and edits. See `shown_layer`.
    pub layer: Layer,
    pub lighting: Lighting,
    /// The active profile's in-game setup step, shown while no key is
    /// selected. Most profiles have none.
    pub setup_note: Option<String>,
    pub selected_key: Option<KeyId>,
    pub combo_text: String,
    pub combo_focused: bool,
    pub recording: bool,
    pub window_id: Option<window::Id>,
    pub after_fix_permissions: bool,
    pub last_error: Option<String>,
    pub hidden: bool,
    pub tab: Tab,
    pub menu_open: bool,
    pub hovered_key: Option<KeyId>,
    pub lighting_saved: bool,
    /// The row of a custom frame that the colour controls edit.
    pub lighting_row: usize,
    /// The preset group the lighting tab's side panel lists.
    pub preset_group: PresetGroup,
    /// `Some` while the inline name box at the bottom of the profile list is
    /// open; the text is what has been typed so far.
    pub new_profile_name: Option<String>,
    /// How the active profile reads the thumbstick.
    pub stick_mode: StickMode,
    /// How far the stick is turned, the same for every profile.
    pub stick_rotation_deg: i32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            ipc_tx: None,
            phase: Phase::Connecting,
            profiles: Vec::new(),
            active_profile_id: None,
            selected_profile_id: None,
            device_present: false,
            ever_present: false,
            model: None,
            openrazer: false,
            evdev_ok: true,
            uinput_ok: true,
            grab_conflict: None,
            bindings: BTreeMap::new(),
            hypershift_bindings: BTreeMap::new(),
            layer: Layer::Base,
            lighting: Lighting {
                effect: LightingEffect::None,
                brightness: theme::DEFAULT_BRIGHTNESS,
                color: None,
                frame: None,
            },
            setup_note: None,
            selected_key: None,
            combo_text: String::new(),
            combo_focused: false,
            recording: false,
            window_id: None,
            after_fix_permissions: false,
            last_error: None,
            hidden: false,
            tab: Tab::default(),
            menu_open: false,
            hovered_key: None,
            lighting_saved: false,
            lighting_row: HOME_ROW,
            preset_group: PresetGroup::WorldOfWarcraft,
            new_profile_name: None,
            stick_mode: StickMode::default(),
            stick_rotation_deg: 0,
        }
    }
}

impl App {
    pub fn theme(&self) -> Theme {
        theme::theme()
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        crate::view::view(self)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            client::subscription(),
            window::close_requests().map(Message::CloseRequested),
            event::listen_with(|event, _status, _id| match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    Some(Message::ComboKey { key, modifiers })
                }
                Event::Mouse(mouse::Event::ButtonPressed(_)) => Some(Message::RefreshComboFocus),
                _ => None,
            }),
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IpcReady(tx) => {
                self.ipc_tx = Some(tx);
                Task::none()
            }
            Message::DaemonStarting => {
                self.phase = Phase::Connecting;
                Task::none()
            }
            Message::DaemonFailed(message) => {
                self.phase = Phase::FailedStart;
                self.last_error = Some(message);
                Task::none()
            }
            Message::DaemonConnected => {
                self.phase = Phase::Running;
                self.send(Method::GetStatus, json!({}));
                self.send(Method::ListProfiles, json!({}));
                Task::none()
            }
            Message::DaemonStopping => iced::exit(),
            Message::IpcResponse {
                method,
                ok,
                result,
                error,
            } => {
                if !ok {
                    if let Some(err) = error {
                        self.apply_error(err.code, Some(err.message));
                    }
                    return Task::none();
                }
                if let Some(result) = result {
                    self.handle_result(method, result);
                }
                Task::none()
            }
            Message::IpcEvent { method, params } => self.handle_event(method, params),
            Message::SelectProfile(id) => {
                self.lighting_saved = false;
                self.combo_focused = false;
                self.selected_profile_id = Some(id.clone());
                self.send(Method::ApplyProfile, json!({ "id": id }));
                Task::none()
            }
            Message::SelectKey(id) => {
                self.selected_key = Some(id);
                self.combo_focused = false;
                Task::none()
            }
            Message::SelectLayer(layer) => {
                self.layer = layer;
                if self.shown_layer() == Layer::Hypershift
                    && self.selected_key == Some(KeyId::Hypershift)
                {
                    self.selected_key = None;
                }
                Task::none()
            }
            Message::Record => {
                if let Some(key_id) = self.selected_key {
                    self.send(
                        Method::StartRecord,
                        json!({
                            "key_id": key_id,
                            "layer": self.shown_layer(),
                            "timeout_ms": RECORD_TIMEOUT_MS,
                        }),
                    );
                    self.recording = true;
                }
                Task::none()
            }
            Message::StopRecord => {
                self.send(Method::StopRecord, json!({}));
                self.recording = false;
                Task::none()
            }
            Message::ClearBinding => {
                if let (Some(profile_id), Some(key_id)) = (self.profile_id(), self.selected_key) {
                    let layer = self.shown_layer();
                    self.shown_bindings_mut().remove(&key_id);
                    self.send(
                        Method::ClearBinding,
                        json!({ "profile_id": profile_id, "key_id": key_id, "layer": layer }),
                    );
                }
                Task::none()
            }
            Message::ComboChanged(text) => {
                self.combo_text = text;
                query_focused_id().map(Message::ComboFocusChanged)
            }
            Message::RefreshComboFocus => query_focused_id().map(Message::ComboFocusChanged),
            Message::ComboFocusChanged(id) => {
                self.combo_focused = is_combo_id(id.as_ref());
                Task::none()
            }
            Message::ComboKey { key, modifiers } => {
                if self.recording {
                    self.apply_captured_key(&key, modifiers, self.combo_focused);
                    return Task::none();
                }
                query_focused_id().map(move |focused| Message::ComboKeyResolved {
                    key: key.clone(),
                    modifiers,
                    focused,
                })
            }
            Message::ComboKeyResolved {
                key,
                modifiers,
                focused,
            } => {
                // Escape in the new-profile box closes it. Any other key there
                // is typing, not a binding, and must never reach the combo
                // capture below.
                if is_new_profile_id(focused.as_ref()) {
                    if matches!(key, Key::Named(Named::Escape)) {
                        self.new_profile_name = None;
                    }
                    return Task::none();
                }
                self.combo_focused = is_combo_id(focused.as_ref());
                self.apply_captured_key(&key, modifiers, self.combo_focused);
                Task::none()
            }
            Message::HoldRepeat(enabled) => {
                if let Some(action) = self.selected_action() {
                    let next = if enabled {
                        match action {
                            Action::HoldRepeat { .. } => action,
                            Action::Key { .. } => Action::HoldRepeat {
                                inner: Box::new(action),
                                rate_ms: HOLD_REPEAT_RATE_DEFAULT_MS,
                            },
                            _ => action,
                        }
                    } else if let Action::HoldRepeat { inner, .. } = action {
                        *inner
                    } else {
                        action
                    };
                    self.apply_action(next);
                }
                Task::none()
            }
            Message::MousePick(target) => {
                if self.recording {
                    self.send(Method::SubmitRecord, submit_record_mouse_params(&target));
                    self.recording = false;
                } else {
                    self.apply_action(Action::Mouse { target });
                }
                Task::none()
            }
            Message::KeyPick(key) => {
                if self.recording {
                    self.send(Method::SubmitRecord, submit_record_key_params(key, &[]));
                    self.recording = false;
                } else {
                    self.apply_action(Action::Key {
                        key,
                        modifiers: vec![],
                    });
                }
                Task::none()
            }
            Message::AddMacroStep => {
                let mut steps = match self.selected_action() {
                    Some(Action::Macro { steps }) => steps,
                    _ => Vec::new(),
                };
                if steps.len() < MACRO_MAX_STEPS {
                    steps.push(MacroStep {
                        kind: MacroKind::Key {
                            key: KeyToken::A,
                            modifiers: vec![],
                        },
                        edge: Edge::Tap,
                        delay_ms: 0,
                    });
                    self.apply_action(Action::Macro { steps });
                }
                Task::none()
            }
            Message::DeleteMacroStep(index) => {
                if let Some(Action::Macro { mut steps }) = self.selected_action() {
                    if index < steps.len() {
                        steps.remove(index);
                    }
                    if steps.is_empty() {
                        if let (Some(profile_id), Some(key_id)) =
                            (self.profile_id(), self.selected_key)
                        {
                            let layer = self.shown_layer();
                            self.shown_bindings_mut().remove(&key_id);
                            self.send(
                                Method::ClearBinding,
                                json!({
                                    "profile_id": profile_id,
                                    "key_id": key_id,
                                    "layer": layer,
                                }),
                            );
                        }
                    } else {
                        self.apply_action(Action::Macro { steps });
                    }
                }
                Task::none()
            }
            Message::MacroDelay(index, text) => {
                if let Some(Action::Macro { mut steps }) = self.selected_action() {
                    if let Some(step) = steps.get_mut(index) {
                        let parsed = text.parse::<u32>().unwrap_or(step.delay_ms);
                        step.delay_ms = parsed.min(MACRO_MAX_DELAY_MS);
                    }
                    self.apply_action(Action::Macro { steps });
                }
                Task::none()
            }
            Message::LightingEffect(choice) => {
                let effect = choice.0;
                self.lighting.effect = effect;
                if uses_color(effect) && self.lighting.color.is_none() {
                    self.lighting.color = Some(theme::DEFAULT_LIGHT_COLOR);
                }
                if effect == LightingEffect::Custom && self.lighting.frame.is_none() {
                    let rgb = self.lighting.color.unwrap_or(theme::DEFAULT_LIGHT_COLOR);
                    self.lighting.frame = Some(solid_frame(rgb));
                }
                self.send_lighting();
                self.lighting_saved = true;
                Task::none()
            }
            Message::LightingBrightness(value) => {
                self.lighting.brightness = value.min(theme::BRIGHTNESS_MAX);
                self.send_lighting();
                self.lighting_saved = true;
                Task::none()
            }
            Message::LightingColor(channel, value) => {
                let mut rgb = self.edited_color();
                if channel < 3 {
                    rgb[channel] = value;
                }
                self.set_edited_color(rgb);
                self.send_lighting();
                self.lighting_saved = true;
                Task::none()
            }
            Message::StickModePick(StickChoice(mode)) => {
                self.stick_mode = mode;
                if let Some(id) = self.profile_id() {
                    self.send(
                        Method::SetStickMode,
                        json!({ "profile_id": id, "mode": mode }),
                    );
                }
                Task::none()
            }
            Message::StickRotationChanged(degrees) => {
                self.stick_rotation_deg =
                    degrees.clamp(-STICK_ROTATION_MAX_DEG, STICK_ROTATION_MAX_DEG);
                Task::none()
            }
            Message::StickRotationReleased => {
                self.send(
                    Method::SetStickRotation,
                    json!({ "degrees": self.stick_rotation_deg }),
                );
                Task::none()
            }
            Message::RevertProfile => {
                if let Some(id) = self.profile_id() {
                    self.send(Method::RevertProfile, json!({ "id": id }));
                }
                Task::none()
            }
            Message::FixPermissions => {
                self.menu_open = false;
                Task::perform(client::run_fix_permissions(), Message::FixPermissionsDone)
            }
            Message::FixPermissionsDone(outcome) => {
                match outcome {
                    FixPermissionsOutcome::Success => {
                        self.after_fix_permissions = true;
                        self.send(Method::GetStatus, json!({}));
                    }
                    FixPermissionsOutcome::Cancelled | FixPermissionsOutcome::Failed => {}
                }
                Task::none()
            }
            Message::Quit => {
                self.menu_open = false;
                self.send(Method::QuitDaemon, json!({}));
                iced::exit()
            }
            Message::CloseRequested(id) => {
                self.window_id = Some(id);
                self.hidden = true;
                match close_action(session_is_wayland()) {
                    CloseAction::HideToTray => Task::batch([
                        window::change_mode(id, Mode::Hidden),
                        window::minimize(id, true),
                    ]),
                    CloseAction::ExitUi => iced::exit(),
                }
            }
            Message::WindowId(id) => {
                self.window_id = id;
                Task::none()
            }
            Message::SelectTab(tab) => {
                self.tab = tab;
                self.menu_open = false;
                Task::none()
            }
            Message::ToggleMenu => {
                self.menu_open = !self.menu_open;
                Task::none()
            }
            Message::HoverKey(id) => {
                self.hovered_key = id;
                Task::none()
            }
            Message::LightingPreset(rgb) => {
                self.set_edited_color(rgb);
                self.send_lighting();
                self.lighting_saved = true;
                Task::none()
            }
            Message::LightingRow(row) => {
                self.lighting_row = row.min(FRAME_ROWS - 1);
                // Picking a row on a pad lit in one colour starts a custom
                // frame in that colour, so the row can then take its own.
                if self.lighting.effect != LightingEffect::Custom
                    && self
                        .offered_effects()
                        .contains(&EffectChoice(LightingEffect::Custom))
                {
                    return self.update(Message::LightingEffect(EffectChoice(
                        LightingEffect::Custom,
                    )));
                }
                Task::none()
            }
            Message::PresetGroupPick(group) => {
                self.preset_group = group;
                Task::none()
            }
            Message::LightFramePreset(id) => {
                if let Some(preset) = preset_by_id(id) {
                    self.lighting.effect = LightingEffect::Custom;
                    self.lighting.frame = Some(preset.frame());
                    self.send_lighting();
                    self.lighting_saved = true;
                }
                Task::none()
            }
            // The window has no system title bar, so the header does the work
            // a title bar normally would. Each of these resolves the window id
            // at call time, because it is not known until the window opens.
            Message::DragWindow => self.with_window(window::drag),
            Message::MinimizeWindow => {
                self.menu_open = false;
                self.with_window(|id| window::minimize(id, true))
            }
            Message::ToggleMaximize => {
                self.menu_open = false;
                self.with_window(window::toggle_maximize)
            }
            Message::CloseWindow => {
                self.menu_open = false;
                // Route through CloseRequested so the header's close button and
                // the compositor's own close both take the same path: hide to
                // tray on X11, exit the UI on Wayland.
                self.with_window(|id| Task::done(Message::CloseRequested(id)))
            }
            Message::StartNewProfile => {
                self.menu_open = false;
                self.new_profile_name = Some(String::new());
                text_input::focus(text_input::Id::new(NEW_PROFILE_INPUT_ID))
            }
            Message::NewProfileNameChanged(text) => {
                if self.new_profile_name.is_some() {
                    self.new_profile_name = Some(text);
                }
                Task::none()
            }
            Message::SubmitNewProfile => {
                // A blank name is a no-op, not an error: the box stays open so
                // the user can type one. The daemon copies from `copy_from`,
                // which is whatever is selected, so the new profile starts as
                // that layout under the new name.
                let Some(name) = self.new_profile_name.as_deref() else {
                    return Task::none();
                };
                let name = name.trim();
                if name.is_empty() {
                    return Task::none();
                }
                let mut params = json!({ "name": name });
                if let Some(source) = self.profile_id() {
                    params["copy_from"] = json!(source);
                }
                self.send(Method::CreateProfile, params);
                self.new_profile_name = None;
                Task::none()
            }
            Message::CancelNewProfile => {
                self.new_profile_name = None;
                Task::none()
            }
            Message::DeleteProfile(id) => {
                self.send(Method::DeleteProfile, json!({ "id": id }));
                Task::none()
            }
            Message::NextProfilePick => {
                self.apply_action(Action::NextProfile);
                Task::none()
            }
            Message::SwitchProfilePick(choice) => {
                self.apply_action(Action::SwitchProfile { profile: choice.id });
                Task::none()
            }
        }
    }

    /// Runs a window task against the current window, looking the id up when
    /// it has not been cached yet.
    fn with_window(
        &self,
        task: impl Fn(window::Id) -> Task<Message> + Send + 'static,
    ) -> Task<Message> {
        match self.window_id {
            Some(id) => task(id),
            None => window::get_latest().then(move |found| match found {
                Some(id) => task(id),
                None => Task::none(),
            }),
        }
    }

    pub fn banner(&self) -> Option<Banner> {
        if self.hidden {
            return None;
        }
        match self.phase {
            Phase::Connecting => Some(Banner::Starting),
            Phase::FailedStart => Some(Banner::CouldNotStart(
                self.last_error.clone().unwrap_or_else(|| {
                    theme::could_not_start_message(theme::START_REASON_TRAY_DID_NOT_START)
                }),
            )),
            Phase::Running => {
                if let Some(stored) = &self.grab_conflict {
                    Some(Banner::GrabConflict {
                        name: grab_conflict_name(stored),
                    })
                } else if !self.evdev_ok || !self.uinput_ok {
                    if self.after_fix_permissions && !self.evdev_ok {
                        Some(Banner::SignOut)
                    } else if self.after_fix_permissions {
                        Some(Banner::UnplugAfterFix)
                    } else {
                        Some(Banner::Permission)
                    }
                } else if !self.device_present {
                    if self.after_fix_permissions {
                        Some(Banner::UnplugAfterFix)
                    } else if self.ever_present {
                        Some(Banner::Disconnect)
                    } else {
                        Some(Banner::NoDevice)
                    }
                } else {
                    self.last_error
                        .as_ref()
                        .map(|msg| Banner::Other(msg.clone()))
                }
            }
        }
    }

    /// The name of the active profile, read from the profile rows rather than
    /// from the id, so the status bar shows what the sidebar shows.
    pub fn active_profile_name(&self) -> Option<&str> {
        let id = self.active_profile_id.as_deref()?;
        self.profiles
            .iter()
            .find(|row| row.id == id)
            .map(|row| row.name.as_str())
    }

    /// The status bar text. A lighting write wins over the profile name,
    /// because it is the more recent thing the user did.
    pub fn status_line(&self) -> String {
        if self.lighting_saved {
            return String::from(theme::STATUS_LIGHTING_SAVED);
        }
        match self.active_profile_name() {
            Some(name) => format!("{name}{}", theme::STATUS_APPLIED_SUFFIX),
            None => String::from(theme::STATUS_READY),
        }
    }

    /// The header pill text. A present device with an unknown model reads as
    /// absent, because we cannot name the pad we are drawing.
    pub fn device_pill_text(&self) -> &'static str {
        if !self.device_present {
            return theme::DEVICE_NOT_CONNECTED;
        }
        match self.model {
            Some(DeviceModel::V2) => theme::DEVICE_CONNECTED_V2,
            Some(DeviceModel::Pro) => theme::DEVICE_CONNECTED_PRO,
            Some(DeviceModel::V2Pro) => theme::DEVICE_CONNECTED_V2_PRO,
            None => theme::DEVICE_NOT_CONNECTED,
        }
    }

    pub fn selected_action(&self) -> Option<Action> {
        self.selected_key
            .and_then(|id| self.shown_bindings().get(&id).cloned())
    }

    /// The layer the keypad shows. Only the V2 Pro has Hypershift, so any
    /// other pad always shows its normal bindings.
    pub fn shown_layer(&self) -> Layer {
        if self.model == Some(DeviceModel::V2Pro) {
            self.layer
        } else {
            Layer::Base
        }
    }

    pub fn shown_bindings(&self) -> &BTreeMap<KeyId, Action> {
        self.bindings_for(self.shown_layer())
    }

    fn shown_bindings_mut(&mut self) -> &mut BTreeMap<KeyId, Action> {
        match self.shown_layer() {
            Layer::Base => &mut self.bindings,
            Layer::Hypershift => &mut self.hypershift_bindings,
        }
    }

    fn bindings_for(&self, layer: Layer) -> &BTreeMap<KeyId, Action> {
        match layer {
            Layer::Base => &self.bindings,
            Layer::Hypershift => &self.hypershift_bindings,
        }
    }

    /// Stores a binding the daemon recorded, in the layer it was recorded
    /// for, which may no longer be the one on show.
    fn store_recorded(&mut self, result: &Value) {
        let Ok(key_id) =
            serde_json::from_value::<KeyId>(result.get("key_id").cloned().unwrap_or(Value::Null))
        else {
            return;
        };
        let Ok(action) =
            serde_json::from_value::<Action>(result.get("action").cloned().unwrap_or(Value::Null))
        else {
            return;
        };
        let layer = result
            .get("layer")
            .and_then(|v| serde_json::from_value::<Layer>(v.clone()).ok())
            .unwrap_or_default();
        match layer {
            Layer::Base => self.bindings.insert(key_id, action),
            Layer::Hypershift => self.hypershift_bindings.insert(key_id, action),
        };
    }

    /// Every profile the switch-profile picker can jump to, in list order.
    pub fn profile_choices(&self) -> Vec<ProfileChoice> {
        self.profiles
            .iter()
            .map(|row| ProfileChoice {
                id: row.id.clone(),
                name: row.name.clone(),
            })
            .collect()
    }

    /// The picker's current value: the profile the selected key jumps to,
    /// when it is a switch key and that profile still exists.
    pub fn switch_target_choice(&self) -> Option<ProfileChoice> {
        let Some(Action::SwitchProfile { profile }) = self.selected_action() else {
            return None;
        };
        self.profile_choices()
            .into_iter()
            .find(|choice| choice.id == profile)
    }

    pub fn profile_id(&self) -> Option<String> {
        self.active_profile_id
            .clone()
            .or_else(|| self.selected_profile_id.clone())
    }

    pub fn keypad_faded(&self) -> bool {
        self.ever_present && !self.device_present
    }

    pub fn lighting_footer_message(&self) -> Option<&'static str> {
        if self.phase != Phase::Running {
            return None;
        }
        if self.openrazer {
            None
        } else {
            Some(ErrorCode::Lighting.user_message())
        }
    }

    fn send(&self, method: Method, params: Value) {
        if let Some(tx) = &self.ipc_tx {
            let _ = tx.unbounded_send(Outgoing { method, params });
        }
    }

    fn apply_captured_key(
        &mut self,
        key: &Key,
        modifiers: keyboard::Modifiers,
        combo_focused: bool,
    ) {
        let token = map_key_token(key);
        let mods = map_modifiers(modifiers);
        match capture_window_key(self.recording, combo_focused, token, mods) {
            KeyCapture::SubmitRecord { key, modifiers } => {
                self.send(
                    Method::SubmitRecord,
                    submit_record_key_params(key, &modifiers),
                );
                self.recording = false;
                self.combo_text.clear();
            }
            KeyCapture::SetBinding { key, modifiers } => {
                self.combo_text.clear();
                self.apply_action(Action::Key { key, modifiers });
            }
            KeyCapture::Ignore => {}
        }
    }

    fn apply_action(&mut self, action: Action) {
        if let (Some(profile_id), Some(key_id)) = (self.profile_id(), self.selected_key) {
            let layer = self.shown_layer();
            self.shown_bindings_mut().insert(key_id, action.clone());
            self.send(
                Method::SetBinding,
                json!({
                    "profile_id": profile_id,
                    "key_id": key_id,
                    "layer": layer,
                    "action": action,
                }),
            );
        }
    }

    /// The colour the colour controls show: the selected row's for a custom
    /// frame, else the plain colour.
    pub fn edited_color(&self) -> [u8; 3] {
        if self.lighting.effect == LightingEffect::Custom {
            if let Some(rgb) = self
                .lighting
                .frame
                .as_ref()
                .and_then(|frame| frame.get(self.lighting_row))
                .and_then(|row| row.first())
            {
                return *rgb;
            }
        }
        self.lighting.color.unwrap_or(theme::DEFAULT_LIGHT_COLOR)
    }

    fn set_edited_color(&mut self, rgb: [u8; 3]) {
        if self.lighting.effect == LightingEffect::Custom {
            let frame = self.lighting.frame.get_or_insert_with(Vec::new);
            fill_row(frame, self.lighting_row, rgb);
        } else {
            self.lighting.color = Some(rgb);
        }
    }

    /// The effects the lighting tab offers. A custom frame needs a pad whose
    /// LED grid is mapped, so it shows only on the V2 Pro, or when the profile
    /// already has one.
    pub fn offered_effects(&self) -> Vec<EffectChoice> {
        LIGHTING_EFFECTS
            .into_iter()
            .filter(|choice| {
                choice.0 != LightingEffect::Custom
                    || self.model == Some(DeviceModel::V2Pro)
                    || self.lighting.effect == LightingEffect::Custom
            })
            .collect()
    }

    fn send_lighting(&self) {
        if let Some(profile_id) = self.profile_id() {
            self.send(
                Method::SetLighting,
                json!({
                    "profile_id": profile_id,
                    "lighting": self.lighting,
                }),
            );
        }
    }

    fn handle_result(&mut self, method: Option<Method>, result: Value) {
        match method {
            Some(Method::GetStatus) | None => {
                if result.get("device").is_some() {
                    self.apply_status(&result);
                }
                if result.get("profiles").is_some() {
                    self.apply_profiles(&result);
                }
            }
            Some(Method::ListProfiles) => self.apply_profiles(&result),
            Some(Method::ApplyProfile)
            | Some(Method::RevertProfile)
            | Some(Method::CreateProfile)
            | Some(Method::SetBinding)
            | Some(Method::SetStickMode)
            | Some(Method::ClearBinding)
            | Some(Method::SetLighting) => {
                if let Some(id) = result.get("id").and_then(Value::as_str) {
                    self.active_profile_id = Some(id.to_string());
                    self.selected_profile_id = Some(id.to_string());
                    self.load_profile(id);
                }
                self.send(Method::ListProfiles, json!({}));
                self.send(Method::GetStatus, json!({}));
            }
            Some(Method::DeleteProfile) => {
                // The daemon reports what is active now: unchanged if the
                // deleted profile was not the live one, the fallback if it was.
                if let Some(id) = result.get("active_id").and_then(Value::as_str) {
                    self.active_profile_id = Some(id.to_string());
                    self.selected_profile_id = Some(id.to_string());
                    self.load_profile(id);
                }
                self.selected_key = None;
                self.send(Method::ListProfiles, json!({}));
                self.send(Method::GetStatus, json!({}));
            }
            Some(Method::StartRecord) => {
                self.recording = true;
            }
            Some(Method::StopRecord) => {
                self.recording = false;
            }
            Some(Method::SubmitRecord) => {
                self.recording = false;
                self.store_recorded(&result);
            }
            _ => {}
        }
    }

    fn handle_event(&mut self, method: EventMethod, params: Value) -> Task<Message> {
        match method {
            EventMethod::DeviceChanged => {
                self.apply_device(&params);
                Task::none()
            }
            EventMethod::Recorded => {
                self.recording = false;
                self.store_recorded(&params);
                Task::none()
            }
            EventMethod::RecordCancelled => {
                self.recording = false;
                Task::none()
            }
            EventMethod::Error => {
                if let Ok(code) = serde_json::from_value::<ErrorCode>(
                    params.get("code").cloned().unwrap_or(Value::Null),
                ) {
                    let message = params
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    self.apply_error(code, message);
                }
                Task::none()
            }
            EventMethod::ProfileApplied => {
                if let Some(id) = params.get("id").and_then(Value::as_str) {
                    self.active_profile_id = Some(id.to_string());
                    self.load_profile(id);
                }
                self.send(Method::ListProfiles, json!({}));
                Task::none()
            }
            EventMethod::FocusWindow => self.show_window(),
            EventMethod::DaemonStopping => iced::exit(),
        }
    }

    fn show_window(&mut self) -> Task<Message> {
        self.hidden = false;
        if let Some(id) = self.window_id {
            return Task::batch([
                window::change_mode(id, Mode::Windowed),
                window::minimize(id, false),
                window::gain_focus(id),
            ]);
        }
        window::get_latest().then(|id| {
            if let Some(id) = id {
                Task::batch([
                    window::change_mode(id, Mode::Windowed),
                    window::minimize(id, false),
                    window::gain_focus(id),
                ])
            } else {
                Task::none()
            }
        })
    }

    fn apply_status(&mut self, result: &Value) {
        if let Some(device) = result.get("device") {
            self.apply_device(device);
        }
        self.openrazer = result
            .get("openrazer")
            .and_then(|v| v.get("available"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if let Some(id) = result.get("active_profile_id").and_then(Value::as_str) {
            self.active_profile_id = Some(id.to_string());
            self.load_profile(id);
        }
        if let Some(degrees) = result
            .get("stick")
            .and_then(|v| v.get("rotation_deg"))
            .and_then(Value::as_i64)
        {
            self.stick_rotation_deg = degrees as i32;
        }
        self.recording = result
            .get("record")
            .and_then(|v| v.get("active"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.uinput_ok = result
            .get("permissions")
            .and_then(|v| v.get("uinput"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        self.evdev_ok = result
            .get("permissions")
            .and_then(|v| v.get("evdev"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        self.grab_conflict = match result.get("grab_conflict") {
            Some(Value::String(name)) => Some(name.clone()),
            _ => None,
        };
        if self.device_present && self.evdev_ok && self.uinput_ok {
            self.after_fix_permissions = false;
        }
    }

    fn apply_device(&mut self, device: &Value) {
        self.device_present = device
            .get("present")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if self.device_present {
            self.ever_present = true;
        }
        let model = device
            .get("model")
            .and_then(|v| serde_json::from_value::<DeviceModel>(v.clone()).ok());
        let changed = model != self.model;
        self.model = model;
        // A profile can carry its own layout for one model, so the keys to
        // show change with the pad.
        if changed {
            if let Some(id) = self.active_profile_id.clone() {
                self.load_profile(&id);
            }
        }
    }

    fn apply_profiles(&mut self, result: &Value) {
        #[derive(Deserialize)]
        struct Row {
            id: String,
            name: String,
            is_active: bool,
            can_revert: bool,
            // Older daemons do not send this; a missing flag means "not
            // deletable", which is the safe reading.
            #[serde(default)]
            can_delete: bool,
        }
        let rows: Vec<Row> = result
            .get("profiles")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let mut ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        ids = opentartarus_core::labels::profile_row_order(&ids);
        let mut by_id: BTreeMap<String, Row> =
            rows.into_iter().map(|r| (r.id.clone(), r)).collect();
        self.profiles = ids
            .into_iter()
            .filter_map(|id| by_id.remove(&id))
            .map(|r| ProfileRow {
                color: read_profile(&r.id).and_then(|profile| profile.lighting.main_color()),
                id: r.id,
                name: r.name,
                is_active: r.is_active,
                can_revert: r.can_revert,
                can_delete: r.can_delete,
            })
            .collect();
    }

    fn load_profile(&mut self, id: &str) {
        if let Some(profile) = read_profile(id) {
            self.bindings = profile.layout(self.model).clone();
            self.hypershift_bindings = profile.hypershift;
            self.lighting = profile.lighting;
            self.setup_note = profile.setup_note;
            self.stick_mode = profile.stick_mode;
        }
    }

    fn apply_error(&mut self, code: ErrorCode, message: Option<String>) {
        let display = message
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| code.user_message().to_string());
        match code {
            ErrorCode::Permission => {
                self.evdev_ok = false;
            }
            ErrorCode::Disconnect => {
                self.device_present = false;
            }
            ErrorCode::Lighting => {
                self.openrazer = false;
            }
            ErrorCode::GrabConflict => {
                self.grab_conflict = Some(display);
            }
            ErrorCode::AlreadyRunning => {}
            ErrorCode::InvalidProfile
            | ErrorCode::UnknownKey
            | ErrorCode::RecordBusy
            | ErrorCode::NotFound
            | ErrorCode::Io => {
                self.last_error = Some(display);
            }
        }
    }
}

fn grab_conflict_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == ErrorCode::GrabConflict.user_message() {
        return None;
    }
    if trimmed.contains('/') {
        return None;
    }
    Some(trimmed.to_string())
}

pub fn uses_color(effect: LightingEffect) -> bool {
    matches!(
        effect,
        LightingEffect::Static
            | LightingEffect::Breath
            | LightingEffect::Reactive
            | LightingEffect::Starlight
    )
}

pub fn effect_name(effect: LightingEffect) -> &'static str {
    match effect {
        LightingEffect::Static => "static",
        LightingEffect::Wave => "wave",
        LightingEffect::Spectrum => "spectrum",
        LightingEffect::Breath => "breath",
        LightingEffect::Reactive => "reactive",
        LightingEffect::Starlight => "starlight",
        LightingEffect::None => "none",
        LightingEffect::Custom => "custom",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectChoice(pub LightingEffect);

impl std::fmt::Display for EffectChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(effect_name(self.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StickChoice(pub StickMode);

impl std::fmt::Display for StickChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            StickMode::Keys8 => theme::STICK_MODE_KEYS8,
            StickMode::Keys4 => theme::STICK_MODE_KEYS4,
            StickMode::AnalogLeft => theme::STICK_MODE_ANALOG_LEFT,
            StickMode::AnalogRight => theme::STICK_MODE_ANALOG_RIGHT,
        })
    }
}

impl App {
    /// Whether the inspector shows the thumbstick settings: a stick key is
    /// selected on a pad that has a stick.
    pub fn shows_stick_settings(&self) -> bool {
        match (self.selected_key, self.model) {
            (Some(key), Some(model)) => key.is_stick() && model.has_key(key),
            _ => false,
        }
    }
}

pub const LIGHTING_EFFECTS: [EffectChoice; 8] = [
    EffectChoice(LightingEffect::Static),
    EffectChoice(LightingEffect::Wave),
    EffectChoice(LightingEffect::Spectrum),
    EffectChoice(LightingEffect::Breath),
    EffectChoice(LightingEffect::Reactive),
    EffectChoice(LightingEffect::Starlight),
    EffectChoice(LightingEffect::Custom),
    EffectChoice(LightingEffect::None),
];

/// A profile as the switch-profile picker offers it: shown by name, keyed by
/// id. `pick_list` needs `Display` for the row and equality for the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileChoice {
    pub id: String,
    pub name: String,
}

impl std::fmt::Display for ProfileChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

fn read_profile(id: &str) -> Option<Profile> {
    let paths = Paths::from_env();
    store::read_user_profile(&paths, id)
        .or_else(|_| pack::shipped_profile(id))
        .ok()
}

fn map_modifiers(modifiers: keyboard::Modifiers) -> Vec<Modifier> {
    let mut out = Vec::new();
    if modifiers.control() {
        out.push(Modifier::Ctrl);
    }
    if modifiers.shift() {
        out.push(Modifier::Shift);
    }
    if modifiers.alt() {
        out.push(Modifier::Alt);
    }
    if modifiers.logo() {
        out.push(Modifier::Super);
    }
    out
}

fn map_key_token(key: &Key) -> Option<KeyToken> {
    match key {
        Key::Character(c) => match c.to_ascii_lowercase().as_str() {
            "a" => Some(KeyToken::A),
            "b" => Some(KeyToken::B),
            "c" => Some(KeyToken::C),
            "d" => Some(KeyToken::D),
            "e" => Some(KeyToken::E),
            "f" => Some(KeyToken::F),
            "g" => Some(KeyToken::G),
            "h" => Some(KeyToken::H),
            "i" => Some(KeyToken::I),
            "j" => Some(KeyToken::J),
            "k" => Some(KeyToken::K),
            "l" => Some(KeyToken::L),
            "m" => Some(KeyToken::M),
            "n" => Some(KeyToken::N),
            "o" => Some(KeyToken::O),
            "p" => Some(KeyToken::P),
            "q" => Some(KeyToken::Q),
            "r" => Some(KeyToken::R),
            "s" => Some(KeyToken::S),
            "t" => Some(KeyToken::T),
            "u" => Some(KeyToken::U),
            "v" => Some(KeyToken::V),
            "w" => Some(KeyToken::W),
            "x" => Some(KeyToken::X),
            "y" => Some(KeyToken::Y),
            "z" => Some(KeyToken::Z),
            "0" => Some(KeyToken::Num0),
            "1" => Some(KeyToken::Num1),
            "2" => Some(KeyToken::Num2),
            "3" => Some(KeyToken::Num3),
            "4" => Some(KeyToken::Num4),
            "5" => Some(KeyToken::Num5),
            "6" => Some(KeyToken::Num6),
            "7" => Some(KeyToken::Num7),
            "8" => Some(KeyToken::Num8),
            "9" => Some(KeyToken::Num9),
            "-" => Some(KeyToken::Minus),
            "=" => Some(KeyToken::Equal),
            "[" => Some(KeyToken::LeftBrace),
            "]" => Some(KeyToken::RightBracket),
            "\\" => Some(KeyToken::Backslash),
            ";" => Some(KeyToken::Semicolon),
            "'" => Some(KeyToken::Apostrophe),
            "`" => Some(KeyToken::Grave),
            "," => Some(KeyToken::Comma),
            "." => Some(KeyToken::Dot),
            "/" => Some(KeyToken::Slash),
            _ => None,
        },
        Key::Named(Named::Escape) => Some(KeyToken::Escape),
        Key::Named(Named::Tab) => Some(KeyToken::Tab),
        Key::Named(Named::Backspace) => Some(KeyToken::Backspace),
        Key::Named(Named::Enter) => Some(KeyToken::Enter),
        Key::Named(Named::Space) => Some(KeyToken::Space),
        Key::Named(Named::Insert) => Some(KeyToken::Insert),
        Key::Named(Named::Delete) => Some(KeyToken::Delete),
        Key::Named(Named::Home) => Some(KeyToken::Home),
        Key::Named(Named::End) => Some(KeyToken::End),
        Key::Named(Named::PageUp) => Some(KeyToken::PageUp),
        Key::Named(Named::PageDown) => Some(KeyToken::PageDown),
        Key::Named(Named::ArrowUp) => Some(KeyToken::Up),
        Key::Named(Named::ArrowDown) => Some(KeyToken::Down),
        Key::Named(Named::ArrowLeft) => Some(KeyToken::Left),
        Key::Named(Named::ArrowRight) => Some(KeyToken::Right),
        Key::Named(Named::F1) => Some(KeyToken::F1),
        Key::Named(Named::F2) => Some(KeyToken::F2),
        Key::Named(Named::F3) => Some(KeyToken::F3),
        Key::Named(Named::F4) => Some(KeyToken::F4),
        Key::Named(Named::F5) => Some(KeyToken::F5),
        Key::Named(Named::F6) => Some(KeyToken::F6),
        Key::Named(Named::F7) => Some(KeyToken::F7),
        Key::Named(Named::F8) => Some(KeyToken::F8),
        Key::Named(Named::F9) => Some(KeyToken::F9),
        Key::Named(Named::F10) => Some(KeyToken::F10),
        Key::Named(Named::F11) => Some(KeyToken::F11),
        Key::Named(Named::F12) => Some(KeyToken::F12),
        Key::Named(Named::Shift)
        | Key::Named(Named::Control)
        | Key::Named(Named::Alt)
        | Key::Named(Named::Super)
        | Key::Named(Named::Meta) => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client;
    use crate::keys::new_profile_widget_id;
    use opentartarus_core::ipc::EventMethod;
    use std::path::Path;

    fn running_app() -> App {
        App {
            phase: Phase::Running,
            device_present: true,
            ever_present: true,
            ..App::default()
        }
    }

    #[test]
    fn grab_conflict_error_does_not_treat_spec_sentence_as_name() {
        let mut app = running_app();
        let sentence = ErrorCode::GrabConflict.user_message();
        let _ = app.update(Message::IpcEvent {
            method: EventMethod::Error,
            params: json!({
                "code": "grab_conflict",
                "message": sentence,
            }),
        });
        assert_eq!(app.banner(), Some(Banner::GrabConflict { name: None }));
    }

    #[test]
    fn grab_conflict_appends_only_a_process_name() {
        let mut app = running_app();
        let _ = app.update(Message::IpcEvent {
            method: EventMethod::Error,
            params: json!({
                "code": "grab_conflict",
                "message": "openrazer-daemon",
            }),
        });
        assert_eq!(
            app.banner(),
            Some(Banner::GrabConflict {
                name: Some("openrazer-daemon".into()),
            })
        );
    }

    #[test]
    fn grab_conflict_status_path_is_not_a_name() {
        let mut app = running_app();
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::GetStatus),
            ok: true,
            result: Some(json!({
                "device": { "present": true, "model": "v2", "vid": "1532", "pid": "022b" },
                "openrazer": { "available": true },
                "active_profile_id": null,
                "record": { "active": false, "key_id": null, "deadline_ms": null },
                "permissions": { "uinput": true, "evdev": true },
                "grab_conflict": "/dev/input/event270",
            })),
            error: None,
        });
        assert_eq!(app.banner(), Some(Banner::GrabConflict { name: None }));
    }

    #[test]
    fn submit_record_error_shows_wire_message() {
        let mut app = running_app();
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::SubmitRecord),
            ok: false,
            result: None,
            error: Some(WireError {
                code: ErrorCode::NotFound,
                message: "Not recording.".into(),
            }),
        });
        assert_eq!(app.last_error.as_deref(), Some("Not recording."));
        assert_eq!(app.banner(), Some(Banner::Other("Not recording.".into())));
    }

    #[test]
    fn present_but_unreadable_is_permission_not_no_device() {
        let mut app = running_app();
        app.device_present = false;
        app.ever_present = false;
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::GetStatus),
            ok: true,
            result: Some(json!({
                "device": { "present": true, "model": "v2", "vid": "1532", "pid": "022b" },
                "openrazer": { "available": true },
                "active_profile_id": null,
                "record": { "active": false, "key_id": null, "deadline_ms": null },
                "permissions": { "uinput": true, "evdev": false },
                "grab_conflict": null,
            })),
            error: None,
        });
        assert_eq!(app.banner(), Some(Banner::Permission));
    }

    #[test]
    fn missing_device_uses_no_device_copy() {
        let mut app = running_app();
        app.device_present = false;
        app.ever_present = false;
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::GetStatus),
            ok: true,
            result: Some(json!({
                "device": { "present": false, "model": null, "vid": null, "pid": null },
                "openrazer": { "available": false },
                "active_profile_id": null,
                "record": { "active": false, "key_id": null, "deadline_ms": null },
                "permissions": { "uinput": true, "evdev": true },
                "grab_conflict": null,
            })),
            error: None,
        });
        assert_eq!(app.banner(), Some(Banner::NoDevice));
    }

    #[test]
    fn quit_sends_quit_daemon() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = running_app();
        app.ipc_tx = Some(tx);
        let _ = app.update(Message::Quit);
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::QuitDaemon);
    }

    #[test]
    fn close_hides_and_does_not_quit_daemon() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = running_app();
        app.ipc_tx = Some(tx);
        app.hidden = true;
        assert_eq!(app.banner(), None);
        assert!(rx.try_recv().is_err());
        assert_eq!(close_action(false), CloseAction::HideToTray);
        assert_eq!(close_action(true), CloseAction::ExitUi);
    }

    #[test]
    fn failed_start_shows_specific_reason_not_lighting_copy() {
        let mut app = App::default();
        let reason =
            client::connect_timeout_message(Path::new("/run/user/1000/opentartarus/daemon.sock"));
        let _ = app.update(Message::DaemonFailed(reason.clone()));
        assert_eq!(app.phase, Phase::FailedStart);
        assert_eq!(app.banner(), Some(Banner::CouldNotStart(reason)));
        assert!(app.lighting_footer_message().is_none());
        assert!(!app.openrazer);
    }

    #[test]
    fn running_without_lighting_backend_uses_lighting_copy() {
        let mut app = running_app();
        app.openrazer = false;
        assert_eq!(
            app.lighting_footer_message(),
            Some(ErrorCode::Lighting.user_message())
        );
    }

    fn app_with_profiles() -> App {
        let mut app = running_app();
        app.profiles = vec![
            ProfileRow {
                id: "default".into(),
                name: "Default".into(),
                is_active: false,
                can_revert: false,
                can_delete: false,
                color: None,
            },
            ProfileRow {
                id: "dota-2".into(),
                name: "Dota 2".into(),
                is_active: true,
                can_revert: true,
                can_delete: false,
                color: None,
            },
        ];
        app.active_profile_id = Some("dota-2".into());
        app
    }

    #[test]
    fn tab_starts_on_keys_and_switching_closes_the_menu() {
        let mut app = running_app();
        assert_eq!(app.tab, Tab::Keys);
        app.menu_open = true;
        let _ = app.update(Message::SelectTab(Tab::Lighting));
        assert_eq!(app.tab, Tab::Lighting);
        assert!(!app.menu_open, "switching tabs must dismiss the menu");
        let _ = app.update(Message::SelectTab(Tab::Keys));
        assert_eq!(app.tab, Tab::Keys);
    }

    #[test]
    fn toggle_menu_flips_and_actions_dismiss_it() {
        let mut app = running_app();
        assert!(!app.menu_open);
        let _ = app.update(Message::ToggleMenu);
        assert!(app.menu_open);
        let _ = app.update(Message::ToggleMenu);
        assert!(!app.menu_open);

        app.menu_open = true;
        let _ = app.update(Message::FixPermissions);
        assert!(!app.menu_open, "Fix permissions must dismiss the menu");
    }

    #[test]
    fn quit_still_quits_the_daemon_from_the_menu() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = running_app();
        app.ipc_tx = Some(tx);
        app.menu_open = true;
        let _ = app.update(Message::Quit);
        assert!(!app.menu_open);
        assert_eq!(rx.try_recv().unwrap().method, Method::QuitDaemon);
    }

    #[test]
    fn hover_sets_and_clears_the_highlighted_key() {
        let mut app = running_app();
        assert_eq!(app.hovered_key, None);
        let _ = app.update(Message::HoverKey(Some(KeyId::Kp07)));
        assert_eq!(app.hovered_key, Some(KeyId::Kp07));
        let _ = app.update(Message::HoverKey(None));
        assert_eq!(app.hovered_key, None);
    }

    #[test]
    fn custom_effect_starts_from_the_current_colour_on_every_key() {
        let mut app = App {
            model: Some(DeviceModel::V2Pro),
            ..App::default()
        };
        app.lighting.color = Some([10, 20, 30]);
        let _ = app.update(Message::LightingEffect(EffectChoice(
            LightingEffect::Custom,
        )));
        assert_eq!(app.lighting.frame, Some(solid_frame([10, 20, 30])));
        assert!(app.lighting_saved);
    }

    #[test]
    fn colour_controls_edit_only_the_picked_row_of_a_frame() {
        let mut app = App::default();
        app.lighting.effect = LightingEffect::Custom;
        app.lighting.frame = Some(solid_frame([0, 0, 0]));
        let _ = app.update(Message::LightingRow(1));
        let _ = app.update(Message::LightingPreset([255, 0, 0]));
        let _ = app.update(Message::LightingColor(2, 99));
        let frame = app.lighting.frame.clone().unwrap();
        assert!(frame[1].iter().all(|c| *c == [255, 0, 99]));
        assert!(frame[0].iter().all(|c| *c == [0, 0, 0]));
        assert_eq!(app.edited_color(), [255, 0, 99]);
        assert_eq!(app.lighting.color, None, "the plain colour stays put");
    }

    #[test]
    fn a_light_preset_switches_to_custom_with_its_frame() {
        let mut app = App::default();
        let _ = app.update(Message::LightFramePreset("wow-paladin"));
        assert_eq!(app.lighting.effect, LightingEffect::Custom);
        let paladin = preset_by_id("wow-paladin").unwrap();
        assert_eq!(app.lighting.frame, Some(paladin.frame()));
        let _ = app.update(Message::LightFramePreset("no-such-preset"));
        assert_eq!(app.lighting.frame, Some(paladin.frame()));
    }

    #[test]
    fn custom_is_offered_on_the_v2_pro_or_when_already_in_use() {
        let has_custom = |app: &App| {
            app.offered_effects()
                .iter()
                .any(|c| c.0 == LightingEffect::Custom)
        };
        let mut app = App {
            model: Some(DeviceModel::V2),
            ..App::default()
        };
        assert!(!has_custom(&app));
        app.model = Some(DeviceModel::V2Pro);
        assert!(has_custom(&app));
        app.model = Some(DeviceModel::V2);
        app.lighting.effect = LightingEffect::Custom;
        assert!(has_custom(&app));
    }

    #[test]
    fn lighting_preset_sets_all_three_channels_in_one_write() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = app_with_profiles();
        app.ipc_tx = Some(tx);
        app.lighting.effect = LightingEffect::Static;
        let _ = app.update(Message::LightingPreset([0x00, 0xb4, 0xff]));
        assert_eq!(app.lighting.color, Some([0x00, 0xb4, 0xff]));
        assert!(app.lighting_saved);
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::SetLighting);
        assert!(
            rx.try_recv().is_err(),
            "a preset must send exactly one write"
        );
    }

    #[test]
    fn every_lighting_change_marks_the_status_line() {
        let mut app = app_with_profiles();
        assert!(!app.lighting_saved);
        let _ = app.update(Message::LightingBrightness(50));
        assert!(app.lighting_saved);
        assert_eq!(app.status_line(), theme::STATUS_LIGHTING_SAVED);
    }

    #[test]
    fn picking_a_profile_replaces_the_lighting_status() {
        let mut app = app_with_profiles();
        app.lighting_saved = true;
        let _ = app.update(Message::SelectProfile("default".into()));
        assert!(!app.lighting_saved);
    }

    #[test]
    fn status_line_falls_back_from_profile_to_ready() {
        let mut app = app_with_profiles();
        assert_eq!(
            app.status_line(),
            format!("Dota 2{}", theme::STATUS_APPLIED_SUFFIX)
        );

        app.active_profile_id = None;
        assert_eq!(app.status_line(), theme::STATUS_READY);

        app.lighting_saved = true;
        assert_eq!(app.status_line(), theme::STATUS_LIGHTING_SAVED);
    }

    #[test]
    fn device_pill_names_the_connected_model() {
        let mut app = running_app();
        app.model = Some(DeviceModel::V2);
        assert_eq!(app.device_pill_text(), theme::DEVICE_CONNECTED_V2);
        app.model = Some(DeviceModel::Pro);
        assert_eq!(app.device_pill_text(), theme::DEVICE_CONNECTED_PRO);
        app.model = Some(DeviceModel::V2Pro);
        assert_eq!(app.device_pill_text(), theme::DEVICE_CONNECTED_V2_PRO);
        app.device_present = false;
        assert_eq!(app.device_pill_text(), theme::DEVICE_NOT_CONNECTED);
        app.device_present = true;
        app.model = None;
        assert_eq!(app.device_pill_text(), theme::DEVICE_NOT_CONNECTED);
    }

    #[test]
    fn active_profile_name_reads_the_row_not_the_id() {
        let app = app_with_profiles();
        assert_eq!(app.active_profile_name(), Some("Dota 2"));
        let empty = running_app();
        assert_eq!(empty.active_profile_name(), None);
    }

    #[test]
    fn the_shipped_pack_gives_each_game_its_own_swatch_colour() {
        // Pure: reads the compiled-in pack, never the filesystem, so a user
        // profile on the developer's machine cannot change the result.
        let default = pack::shipped_profile("default").unwrap();
        assert_eq!(
            default.lighting.color, None,
            "the shipped default profile has no colour"
        );
        let dota = pack::shipped_profile("dota-2").unwrap();
        assert_eq!(dota.lighting.color, Some([200, 40, 40]));

        let mut colours: Vec<[u8; 3]> = pack::SHIPPED_IDS
            .iter()
            .filter_map(|id| pack::shipped_profile(id).ok())
            .filter_map(|profile| profile.lighting.color)
            .collect();
        let total = colours.len();
        colours.sort_unstable();
        colours.dedup();
        assert_eq!(
            colours.len(),
            total,
            "two shipped profiles share a sidebar swatch colour"
        );
    }

    #[test]
    fn starting_a_new_profile_opens_an_empty_name_box_and_closes_the_menu() {
        let mut app = running_app();
        app.menu_open = true;
        assert_eq!(app.new_profile_name, None);
        let _ = app.update(Message::StartNewProfile);
        assert_eq!(app.new_profile_name.as_deref(), Some(""));
        assert!(!app.menu_open);
    }

    #[test]
    fn typing_only_lands_while_the_box_is_open() {
        let mut app = running_app();
        let _ = app.update(Message::NewProfileNameChanged("stray".into()));
        assert_eq!(app.new_profile_name, None, "no box, nothing to type into");
        let _ = app.update(Message::StartNewProfile);
        let _ = app.update(Message::NewProfileNameChanged("My Raid".into()));
        assert_eq!(app.new_profile_name.as_deref(), Some("My Raid"));
    }

    #[test]
    fn submitting_sends_create_with_the_selected_profile_as_the_source() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = app_with_profiles();
        app.ipc_tx = Some(tx);
        let _ = app.update(Message::StartNewProfile);
        let _ = app.update(Message::NewProfileNameChanged("  My Raid  ".into()));
        let _ = app.update(Message::SubmitNewProfile);
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::CreateProfile);
        assert_eq!(outgoing.params["name"], "My Raid", "name is trimmed");
        assert_eq!(
            outgoing.params["copy_from"], "dota-2",
            "copies whatever is active"
        );
        assert_eq!(app.new_profile_name, None, "the box closes on submit");
    }

    #[test]
    fn a_blank_name_does_not_send_and_leaves_the_box_open() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = app_with_profiles();
        app.ipc_tx = Some(tx);
        let _ = app.update(Message::StartNewProfile);
        let _ = app.update(Message::NewProfileNameChanged("   ".into()));
        let _ = app.update(Message::SubmitNewProfile);
        assert!(
            rx.try_recv().is_err(),
            "nothing to create from a blank name"
        );
        assert!(
            app.new_profile_name.is_some(),
            "box stays open to be filled in"
        );
    }

    #[test]
    fn cancel_and_escape_both_close_the_box_without_sending() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = running_app();
        app.ipc_tx = Some(tx);
        let _ = app.update(Message::StartNewProfile);
        let _ = app.update(Message::CancelNewProfile);
        assert_eq!(app.new_profile_name, None);

        let _ = app.update(Message::StartNewProfile);
        let _ = app.update(Message::ComboKeyResolved {
            key: Key::Named(Named::Escape),
            modifiers: keyboard::Modifiers::default(),
            focused: Some(new_profile_widget_id()),
        });
        assert_eq!(app.new_profile_name, None);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn typing_in_the_name_box_never_becomes_a_key_binding() {
        // The combo capture must ignore keys that land in the new-profile box.
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = app_with_profiles();
        app.ipc_tx = Some(tx);
        app.selected_key = Some(KeyId::Kp01);
        let _ = app.update(Message::StartNewProfile);
        let _ = app.update(Message::ComboKeyResolved {
            key: Key::Character("q".into()),
            modifiers: keyboard::Modifiers::default(),
            focused: Some(new_profile_widget_id()),
        });
        assert!(
            rx.try_recv().is_err(),
            "a typed letter must not send SetBinding"
        );
        assert!(!app.bindings.contains_key(&KeyId::Kp01));
    }

    #[test]
    fn delete_sends_the_id_and_the_reply_moves_selection_to_the_survivor() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = app_with_profiles();
        app.ipc_tx = Some(tx);
        app.selected_key = Some(KeyId::Kp05);
        let _ = app.update(Message::DeleteProfile("my-raid-layout".into()));
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::DeleteProfile);
        assert_eq!(outgoing.params["id"], "my-raid-layout");

        let _ = app.update(Message::IpcResponse {
            method: Some(Method::DeleteProfile),
            ok: true,
            result: Some(json!({ "id": "my-raid-layout", "active_id": "default" })),
            error: None,
        });
        assert_eq!(app.active_profile_id.as_deref(), Some("default"));
        assert_eq!(app.selected_profile_id.as_deref(), Some("default"));
        assert_eq!(app.selected_key, None, "the deleted profile's key is gone");
    }

    #[test]
    fn a_create_reply_selects_the_new_profile() {
        let mut app = app_with_profiles();
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::CreateProfile),
            ok: true,
            result: Some(json!({ "id": "my-raid-layout" })),
            error: None,
        });
        assert_eq!(app.active_profile_id.as_deref(), Some("my-raid-layout"));
        assert_eq!(app.selected_profile_id.as_deref(), Some("my-raid-layout"));
    }

    #[test]
    fn rows_carry_the_delete_flag_and_default_it_off() {
        let mut app = running_app();
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::ListProfiles),
            ok: true,
            result: Some(json!({
                "profiles": [
                    { "id": "default", "name": "Default",
                      "is_active": true, "can_revert": true, "can_delete": false },
                    { "id": "mine", "name": "Mine",
                      "is_active": false, "can_revert": false, "can_delete": true },
                    { "id": "old-daemon", "name": "Old",
                      "is_active": false, "can_revert": false }
                ]
            })),
            error: None,
        });
        let by_id = |id: &str| app.profiles.iter().find(|r| r.id == id).unwrap();
        assert!(!by_id("default").can_delete);
        assert!(by_id("mine").can_delete);
        assert!(
            !by_id("old-daemon").can_delete,
            "a missing flag reads as not deletable"
        );
    }

    #[test]
    fn profile_rows_cache_whatever_the_profile_source_reports() {
        // The row's colour must be whatever `read_profile` resolves for that
        // id — the user's own copy when there is one, the shipped pack
        // otherwise. Asserting a hard-coded colour here would read the real
        // XDG config and fail on a machine where the user has edited that
        // profile, which is exactly what happened before this test was
        // rewritten.
        let mut app = running_app();
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::ListProfiles),
            ok: true,
            result: Some(json!({
                "profiles": [
                    { "id": "dota-2", "name": "Dota 2",
                      "is_active": true, "can_revert": true },
                    { "id": "default", "name": "Default",
                      "is_active": false, "can_revert": false },
                ]
            })),
            error: None,
        });
        assert_eq!(app.profiles.len(), 2);
        for row in &app.profiles {
            let expected = read_profile(&row.id).and_then(|profile| profile.lighting.color);
            assert_eq!(row.color, expected, "row {} cached a stale colour", row.id);
        }
    }

    #[test]
    fn an_unknown_profile_id_gets_no_colour_and_does_not_panic() {
        let mut app = running_app();
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::ListProfiles),
            ok: true,
            result: Some(json!({
                "profiles": [
                    { "id": "my-own-thing", "name": "My Own Thing",
                      "is_active": false, "can_revert": false }
                ]
            })),
            error: None,
        });
        assert_eq!(app.profiles.len(), 1);
        assert_eq!(app.profiles[0].color, None);
    }

    #[test]
    fn stick_settings_show_only_for_a_stick_key_on_a_pad_with_a_stick() {
        let mut app = running_app();
        app.selected_key = Some(KeyId::AnalogUp);
        assert!(!app.shows_stick_settings(), "no model known yet");
        app.model = Some(DeviceModel::V2Pro);
        assert!(app.shows_stick_settings());
        app.selected_key = Some(KeyId::StickClick);
        assert!(app.shows_stick_settings());
        app.selected_key = Some(KeyId::Kp01);
        assert!(!app.shows_stick_settings());
        app.selected_key = Some(KeyId::AnalogUp);
        app.model = Some(DeviceModel::Pro);
        assert_eq!(
            app.shows_stick_settings(),
            DeviceModel::Pro.has_key(KeyId::AnalogUp)
        );
    }

    #[test]
    fn the_stick_mode_goes_to_the_profile_and_the_rotation_on_release() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = running_app();
        app.ipc_tx = Some(tx);
        app.active_profile_id = Some("default".into());

        let _ = app.update(Message::StickModePick(StickChoice(StickMode::Keys4)));
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::SetStickMode);
        assert_eq!(outgoing.params["profile_id"], "default");
        assert_eq!(outgoing.params["mode"], "keys4");
        assert_eq!(app.stick_mode, StickMode::Keys4);

        let _ = app.update(Message::StickRotationChanged(-90));
        assert!(rx.try_recv().is_err(), "dragging sends nothing");
        let _ = app.update(Message::StickRotationChanged(500));
        assert_eq!(app.stick_rotation_deg, 180, "clamped to the slider's range");
        let _ = app.update(Message::StickRotationChanged(-90));
        let _ = app.update(Message::StickRotationReleased);
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::SetStickRotation);
        assert_eq!(outgoing.params["degrees"], -90);
    }

    #[test]
    fn status_carries_the_stick_rotation() {
        let mut app = running_app();
        let _ = app.update(Message::IpcResponse {
            method: Some(Method::GetStatus),
            ok: true,
            result: Some(json!({
                "device": { "present": true, "model": "v2pro" },
                "stick": { "rotation_deg": -90 },
            })),
            error: None,
        });
        assert_eq!(app.stick_rotation_deg, -90);
    }

    #[test]
    fn the_hypershift_layer_is_edited_on_its_own_and_only_on_the_v2_pro() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = app_with_profiles();
        app.ipc_tx = Some(tx);
        app.layer = Layer::Hypershift;
        app.model = Some(DeviceModel::V2);
        assert_eq!(app.shown_layer(), Layer::Base, "only the V2 Pro has it");

        app.model = Some(DeviceModel::V2Pro);
        app.selected_key = Some(KeyId::Kp01);
        let _ = app.update(Message::NextProfilePick);
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::SetBinding);
        assert_eq!(outgoing.params["layer"], "hypershift");
        assert_eq!(
            app.hypershift_bindings.get(&KeyId::Kp01),
            Some(&Action::NextProfile)
        );
        assert!(!app.bindings.contains_key(&KeyId::Kp01), "normal untouched");

        let _ = app.update(Message::ClearBinding);
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::ClearBinding);
        assert_eq!(outgoing.params["layer"], "hypershift");
        assert!(app.hypershift_bindings.is_empty());

        // A recording lands in the layer it was started for.
        app.layer = Layer::Base;
        let _ = app.handle_event(
            EventMethod::Recorded,
            json!({
                "key_id": "kp02",
                "layer": "hypershift",
                "action": { "type": "next_profile" },
            }),
        );
        assert!(app.hypershift_bindings.contains_key(&KeyId::Kp02));
        assert!(!app.bindings.contains_key(&KeyId::Kp02));

        app.selected_key = Some(KeyId::Hypershift);
        let _ = app.update(Message::SelectLayer(Layer::Hypershift));
        assert_eq!(app.selected_key, None, "Hypershift has no layer binding");
    }

    #[test]
    fn picking_a_profile_binds_a_jump_and_next_binds_a_cycle() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = app_with_profiles();
        app.ipc_tx = Some(tx);
        app.selected_key = Some(KeyId::Mode);
        assert_eq!(app.switch_target_choice(), None, "nothing bound yet");

        let choice = ProfileChoice {
            id: "default".into(),
            name: "Default".into(),
        };
        let _ = app.update(Message::SwitchProfilePick(choice.clone()));
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::SetBinding);
        assert_eq!(outgoing.params["key_id"], "mode");
        assert_eq!(
            outgoing.params["action"],
            json!({ "type": "switch_profile", "profile": "default" })
        );
        assert_eq!(
            app.switch_target_choice(),
            Some(choice),
            "the picker shows what was picked"
        );

        let _ = app.update(Message::NextProfilePick);
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.params["action"], json!({ "type": "next_profile" }));
        assert_eq!(app.bindings.get(&KeyId::Mode), Some(&Action::NextProfile));
        assert_eq!(
            app.switch_target_choice(),
            None,
            "a cycle names no one profile"
        );
    }

    #[test]
    fn the_picker_lists_every_profile_by_name_in_list_order() {
        let app = app_with_profiles();
        let names: Vec<String> = app
            .profile_choices()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(names, vec!["Default", "Dota 2"]);
    }

    #[test]
    fn a_held_key_chip_binds_the_bare_modifier_or_answers_a_recording() {
        let (tx, mut rx) = mpsc::unbounded();
        let mut app = app_with_profiles();
        app.ipc_tx = Some(tx);
        app.selected_key = Some(KeyId::Kp16);
        let _ = app.update(Message::KeyPick(KeyToken::LeftShift));
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::SetBinding);
        assert_eq!(
            outgoing.params["action"],
            json!({ "type": "key", "key": "leftshift", "modifiers": [] })
        );

        app.recording = true;
        let _ = app.update(Message::KeyPick(KeyToken::LeftCtrl));
        let outgoing = rx.try_recv().unwrap();
        assert_eq!(outgoing.method, Method::SubmitRecord);
        assert_eq!(outgoing.params["key"], "leftctrl");
        assert!(!app.recording, "the chip ends the recording");
    }
}
