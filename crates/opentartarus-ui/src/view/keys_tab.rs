use crate::app::{App, Message};
use crate::keypad::{self, Keypad};
use crate::theme;
use crate::view::widgets::{card_container, tab_button_style, tab_strip};
use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};
use opentartarus_core::types::{DeviceModel, KeyId, Layer};
use std::collections::BTreeMap;

const LAYERS: [(Layer, &str); 2] = [
    (Layer::Base, theme::LAYER_BASE),
    (Layer::Hypershift, theme::LAYER_HYPERSHIFT),
];

/// Switches the keypad between the normal bindings and the Hypershift layer.
fn layer_switch<'a>(shown: Layer) -> Element<'a, Message> {
    let mut strip = row![].spacing(theme::SPACE_XS);
    for (layer, label) in LAYERS {
        strip = strip.push(
            button(text(label).size(theme::TEXT_SMALL))
                .padding([theme::SPACE_XS, theme::SPACE_MD])
                .on_press(Message::SelectLayer(layer))
                .style(tab_button_style(layer == shown)),
        );
    }
    strip.into()
}

pub fn keys_tab(app: &App) -> Element<'_, Message> {
    let layer = app.shown_layer();
    let fallback = match layer {
        Layer::Base => BTreeMap::new(),
        Layer::Hypershift => app
            .bindings
            .iter()
            .filter(|(key, _)| **key != KeyId::Hypershift)
            .map(|(key, action)| (*key, action.clone()))
            .collect(),
    };
    let pad = keypad::widget(Keypad {
        model: app.model,
        bindings: app.shown_bindings().clone(),
        layer,
        fallback,
        selected: app.selected_key,
        faded: app.keypad_faded(),
        hovered: app.hovered_key,
    });
    let hint = match layer {
        Layer::Base => theme::KEYPAD_HINT,
        Layer::Hypershift => theme::HYPERSHIFT_LAYER_HINT,
    };

    let mut body = column![].spacing(theme::SPACE_MD);
    if app.model == Some(DeviceModel::V2Pro) {
        body = body.push(layer_switch(layer));
    }
    container(
        body.push(pad)
            .push(
                text(hint)
                    .size(theme::TEXT_SMALL)
                    .color(theme::COLOR_TEXT_FAINT),
            )
            .padding(theme::SPACE_XL)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(card_container)
    .into()
}

/// The centre column: the tab strip above whichever tab is showing.
pub fn center_panel(app: &App) -> Element<'_, Message> {
    let body: Element<'_, Message> = match app.tab {
        crate::app::Tab::Keys => keys_tab(app),
        crate::app::Tab::Lighting => crate::view::lighting_tab::lighting_tab(app),
    };
    column![tab_strip(app.tab), body]
        .spacing(theme::SPACE_MD)
        .padding(theme::SPACE_LG)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
