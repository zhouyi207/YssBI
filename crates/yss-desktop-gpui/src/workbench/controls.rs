//! Stateless controls shared by native graph property and node editors.
use gpui::{App, ElementId, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use std::rc::Rc;

pub(super) fn choice(
    id: impl Into<ElementId>,
    label: String,
    current: Option<String>,
    options: Vec<(String, String)>,
    disabled: bool,
    on_choose: impl Fn(&String, &mut Window, &mut App) + 'static,
) -> gpui::AnyElement {
    let on_choose = Rc::new(on_choose);
    Button::new(id)
        .small()
        .w_full()
        .label(label)
        .icon(IconName::ChevronDown)
        .disabled(disabled || options.is_empty())
        .dropdown_menu(move |mut menu, _, _| {
            menu = menu.scrollable(true);
            for (value, label) in &options {
                let value = value.clone();
                let choose = on_choose.clone();
                menu = menu.item(
                    PopupMenuItem::new(label.clone())
                        .checked(current.as_ref() == Some(&value))
                        .on_click(move |_, window, cx| choose(&value, window, cx)),
                );
            }
            menu
        })
        .into_any_element()
}

pub(super) fn hint(text: impl Into<String>, cx: &App) -> gpui::Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub(super) fn apply(id: impl Into<ElementId>, disabled: bool) -> Button {
    Button::new(id)
        .small()
        .ghost()
        .icon(IconName::Check)
        .tooltip("应用修改")
        .disabled(disabled)
}

pub(super) fn number(text: &str) -> Result<serde_json::Value, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("请输入数字".into());
    }
    // Integer drafts stay exact in Rust; never fall back to a rounded float on overflow.
    if !text.contains(['.', 'e', 'E']) {
        if let Ok(value) = text.parse::<i64>() {
            return Ok(value.into());
        }
        return text
            .parse::<u64>()
            .map(serde_json::Value::from)
            .map_err(|_| "请输入有效的整数，数值超出支持范围".into());
    }
    text.parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map(serde_json::Value::Number)
        .ok_or_else(|| "请输入有限的数字".into())
}
