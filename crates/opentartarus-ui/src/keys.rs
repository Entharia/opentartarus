use iced::advanced::widget::operation::focusable::Focusable;
use iced::advanced::widget::operation::{Operation, Outcome};
use iced::advanced::widget::Id;
use iced::Rectangle;
use opentartarus_core::types::{KeyToken, Modifier, MouseTarget};
use serde_json::{json, Value};

pub const COMBO_INPUT_ID: &str = "combo";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCapture {
    SubmitRecord {
        key: KeyToken,
        modifiers: Vec<Modifier>,
    },
    SetBinding {
        key: KeyToken,
        modifiers: Vec<Modifier>,
    },
    Ignore,
}

pub fn capture_window_key(
    recording: bool,
    combo_focused: bool,
    token: Option<KeyToken>,
    modifiers: Vec<Modifier>,
) -> KeyCapture {
    let Some(key) = token else {
        return KeyCapture::Ignore;
    };
    if recording {
        return KeyCapture::SubmitRecord { key, modifiers };
    }
    if combo_focused {
        return KeyCapture::SetBinding { key, modifiers };
    }
    KeyCapture::Ignore
}

pub fn submit_record_key_params(key: KeyToken, modifiers: &[Modifier]) -> Value {
    json!({ "key": key, "modifiers": modifiers })
}

pub fn submit_record_mouse_params(target: &MouseTarget) -> Value {
    match target {
        MouseTarget::Button { button } => json!({ "button": button }),
        MouseTarget::Scroll { scroll } => json!({ "scroll": scroll }),
    }
}

pub fn combo_widget_id() -> Id {
    Id::new(COMBO_INPUT_ID)
}

pub fn is_combo_id(id: Option<&Id>) -> bool {
    id == Some(&combo_widget_id())
}

/// The inline name box for a new profile. It has its own id so keystrokes
/// typed into it are never mistaken for a key binding: the combo capture only
/// fires for `COMBO_INPUT_ID`.
pub const NEW_PROFILE_INPUT_ID: &str = "new-profile-name";

pub fn new_profile_widget_id() -> Id {
    Id::new(NEW_PROFILE_INPUT_ID)
}

pub fn is_new_profile_id(id: Option<&Id>) -> bool {
    id == Some(&new_profile_widget_id())
}

/// The name box for saving a lighting preset. Like the new-profile box,
/// typing in it must never reach the combo capture.
pub const SAVE_PRESET_INPUT_ID: &str = "save-preset-name";

pub fn is_save_preset_id(id: Option<&Id>) -> bool {
    id == Some(&Id::new(SAVE_PRESET_INPUT_ID))
}

/// The hex colour box on the lighting tab. Typing there is never a binding.
pub const HEX_INPUT_ID: &str = "hex-colour";

pub fn is_hex_input_id(id: Option<&Id>) -> bool {
    id == Some(&Id::new(HEX_INPUT_ID))
}

pub fn focused_widget_id() -> impl Operation<Option<Id>> {
    struct FocusedId {
        focused: Option<Id>,
    }

    impl Operation<Option<Id>> for FocusedId {
        fn focusable(&mut self, state: &mut dyn Focusable, id: Option<&Id>) {
            if state.is_focused() {
                self.focused = id.cloned();
            }
        }

        fn container(
            &mut self,
            _id: Option<&Id>,
            _bounds: Rectangle,
            operate_on_children: &mut dyn FnMut(&mut dyn Operation<Option<Id>>),
        ) {
            operate_on_children(self);
        }

        fn finish(&self) -> Outcome<Option<Id>> {
            Outcome::Some(self.focused.clone())
        }
    }

    FocusedId { focused: None }
}

pub fn query_focused_id() -> iced::Task<Option<Id>> {
    iced::advanced::widget::operate(focused_widget_id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentartarus_core::types::{MouseButton, ScrollDir};

    #[test]
    fn recording_sends_submit_record_not_set_binding() {
        let capture = capture_window_key(true, false, Some(KeyToken::C), vec![Modifier::Ctrl]);
        assert_eq!(
            capture,
            KeyCapture::SubmitRecord {
                key: KeyToken::C,
                modifiers: vec![Modifier::Ctrl],
            }
        );
        assert_eq!(
            submit_record_key_params(KeyToken::C, &[Modifier::Ctrl]),
            json!({"key":"c","modifiers":["ctrl"]})
        );
        assert_eq!(
            capture_window_key(true, true, Some(KeyToken::C), vec![Modifier::Ctrl]),
            KeyCapture::SubmitRecord {
                key: KeyToken::C,
                modifiers: vec![Modifier::Ctrl],
            }
        );
    }

    #[test]
    fn type_a_combo_only_when_field_focused() {
        assert_eq!(
            capture_window_key(false, true, Some(KeyToken::Q), vec![]),
            KeyCapture::SetBinding {
                key: KeyToken::Q,
                modifiers: vec![],
            }
        );
        assert_eq!(
            capture_window_key(false, false, Some(KeyToken::Q), vec![]),
            KeyCapture::Ignore
        );
    }

    #[test]
    fn recording_mouse_uses_submit_record_params() {
        assert_eq!(
            submit_record_mouse_params(&MouseTarget::Button {
                button: MouseButton::Left
            }),
            json!({"button":"left"})
        );
        assert_eq!(
            submit_record_mouse_params(&MouseTarget::Scroll {
                scroll: ScrollDir::Up
            }),
            json!({"scroll":"up"})
        );
    }

    #[test]
    fn modifier_only_key_is_ignored() {
        assert_eq!(
            capture_window_key(true, false, None, vec![Modifier::Ctrl]),
            KeyCapture::Ignore
        );
    }
}
