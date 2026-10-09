//! Choice values stay in the projection; localized menu entries are built only on open.
use super::{EditorParameterConfiguration, ParameterDraft, ParameterForm, controls};
use crate::text::translate;
use gpui::{AnyElement, Context, IntoElement, div, prelude::*};
use gpui_component::{
    Disableable, Sizable,
    button::Button,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit_assets::IconName;

fn option_label(key: &str, option: &str) -> String {
    let namespace = match (key, option) {
        ("theil_form", "individual" | "grouped") => Some("theil"),
        ("column_match", "by_name" | "by_position")
        | ("join_type", "inner" | "left" | "right" | "full" | "semi" | "anti") => {
            Some("tableComposition")
        }
        ("target_type", "core.categorical") => return translate("conversion.categorical"),
        ("target_type", "core.ordinal") => return translate("conversion.ordinal"),
        ("numeric_mode", "auto" | "integer" | "real")
        | ("datetime_kind", "auto" | "date" | "time" | "datetime")
        | ("datetime_precision", "seconds" | "milliseconds" | "microseconds" | "nanoseconds") => {
            Some("conversion")
        }
        _ => None,
    };
    namespace
        .map(|namespace| translate(&format!("{namespace}.{option}")))
        .unwrap_or_else(|| option.to_owned())
}

impl ParameterForm {
    pub(super) fn render_parameter_choice(
        &self,
        index: usize,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let field = &self.fields[index];
        let current = field
            .model
            .value
            .as_ref()
            .and_then(serde_json::Value::as_str);
        let constant = matches!(field.draft, ParameterDraft::Constant);
        let properties = self.properties.read(cx);
        let (label, empty, unavailable) = if constant {
            let id = current.and_then(|id| id.parse().ok());
            let name = properties
                .names()
                .find(|(key, _)| Some(*key) == id)
                .map(|(_, name)| name);
            (
                name.map(str::to_owned).unwrap_or_else(|| {
                    translate(if current.is_some() {
                        "native.workbench.constantUnavailable"
                    } else {
                        "detail.constants.choose"
                    })
                }),
                properties.names().next().is_none(),
                false,
            )
        } else {
            let options = match &field.model.configuration {
                Some(EditorParameterConfiguration::SelectOptions { options }) => options.as_ref(),
                _ => &[],
            };
            (
                current
                    .map(|value| option_label(field.model.key.as_str(), value))
                    .unwrap_or_else(|| translate("native.workbench.choose")),
                options.is_empty(),
                current.is_some_and(|value| !options.iter().any(|option| option.as_ref() == value)),
            )
        };
        let loading = constant && properties.loading();
        let epoch = self.epoch;
        let owner = cx.entity().downgrade();
        let button = Button::new(("parameter-choice", index))
            .small()
            .w_full()
            .label(label)
            .icon(IconName::ChevronDown)
            .disabled(busy || loading || empty)
            .dropdown_menu(move |mut menu, _, cx| {
                menu = menu.scrollable(true);
                let Some(view) = owner.upgrade() else {
                    return menu;
                };
                let view = view.read(cx);
                if !view.accepts_input(epoch, cx) {
                    return menu;
                }
                let field = &view.fields[index];
                let current = field
                    .model
                    .value
                    .as_ref()
                    .and_then(serde_json::Value::as_str);
                let options: Vec<_> = if constant {
                    let properties = view.properties.read(cx);
                    if properties.loading() {
                        return menu;
                    }
                    properties
                        .names()
                        .map(|(id, name)| (id.to_string(), name.to_owned()))
                        .collect()
                } else {
                    match &field.model.configuration {
                        Some(EditorParameterConfiguration::SelectOptions { options }) => options
                            .iter()
                            .map(|option| {
                                (
                                    option.to_string(),
                                    option_label(field.model.key.as_str(), option),
                                )
                            })
                            .collect(),
                        _ => vec![],
                    }
                };
                for (value, label) in options {
                    let owner = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(current == Some(value.as_str()))
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |view, cx| {
                                    if view.accepts_input(epoch, cx) {
                                        view.commit_parameter(
                                            index,
                                            serde_json::Value::String(value.clone()),
                                            cx,
                                        );
                                    }
                                });
                            }),
                    );
                }
                menu
            });
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(button)
            .when(unavailable, |view| {
                view.child(controls::hint(
                    translate("detail.parameterEditor.unavailableChoice"),
                    cx,
                ))
            })
            .when(empty && !loading, |view| {
                view.child(controls::hint(
                    translate(if constant {
                        "native.workbench.noConstants"
                    } else {
                        "detail.parameterEditor.noColumns"
                    }),
                    cx,
                ))
            })
            .into_any_element()
    }
}
