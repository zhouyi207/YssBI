//! Read presentation only; the original graph editor normalizes and validates writes.
use gpui::{App, ElementId, IntoElement, Window, prelude::*};
use gpui_component::{
    Disableable, Sizable,
    button::Button,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit_assets::IconName;
use yss_data_contract::{DataValue, SemanticType, ValueType};
use yss_graph_document::GraphConstant;

pub(super) fn type_picker(
    id: impl Into<ElementId>,
    label: String,
    current: Option<String>,
    disabled: bool,
    on_choose: impl Fn(&String, &mut Window, &mut App) + 'static,
) -> gpui::AnyElement {
    let on_choose = std::rc::Rc::new(on_choose);
    Button::new(id)
        .small()
        .w_full()
        .label(label)
        .icon(IconName::ChevronDown)
        .disabled(disabled)
        .dropdown_menu(move |menu, _, _| {
            let mut menu = menu.scrollable(true);
            for (value, label) in type_options() {
                let on_choose = on_choose.clone();
                let checked = current.as_ref() == Some(&value);
                menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(
                    move |_, window, cx| {
                        if !checked {
                            on_choose(&value, window, cx);
                        }
                    },
                ));
            }
            menu
        })
        .into_any_element()
}

pub(super) fn json_hint(data_type: &ValueType) -> Option<&'static str> {
    Some(match data_type {
        ValueType::Array(_) => "detail.constantValue.description.Array",
        ValueType::Object => "detail.constantValue.description.Object",
        ValueType::DataFrame => "detail.constantValue.description.DataFrame",
        ValueType::DataSeries(_) => "detail.constantValue.description.DataSeries",
        _ => return None,
    })
}

fn type_options() -> Vec<(String, String)> {
    let base = SemanticType::ALL
        .into_iter()
        .map(ValueType::Scalar)
        .chain([ValueType::Object, ValueType::DataFrame])
        .collect::<Vec<_>>();
    base.iter()
        .cloned()
        .chain(
            base.iter()
                .cloned()
                .map(|value| ValueType::Array(Box::new(value))),
        )
        .chain(
            SemanticType::ALL
                .into_iter()
                .map(|value| ValueType::DataSeries(Box::new(ValueType::Scalar(value)))),
        )
        .map(|value| {
            let name = value.to_string();
            (name.clone(), name)
        })
        .collect()
}

pub(super) fn scalar_text(value: &DataValue) -> Option<String> {
    match value {
        DataValue::Null => Some(String::new()),
        DataValue::Bool(value) => Some(value.to_string()),
        DataValue::Integer(value) => Some(value.to_string()),
        DataValue::Unsigned(value) => Some(value.to_string()),
        DataValue::Decimal(value) => Some(value.as_str().to_owned()),
        DataValue::String(value) if value.len() <= 4096 => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn initial_value(data_type: &ValueType) -> &'static str {
    match data_type {
        ValueType::Scalar(SemanticType::Numeric) => "0",
        ValueType::Scalar(SemanticType::Binary) => "false",
        ValueType::Scalar(_) => "",
        ValueType::Array(_) => "[]",
        ValueType::DataSeries(_) => "{\"value\":[]}",
        ValueType::DataFrame => "{}",
        _ => "{}",
    }
}

pub(super) fn structured_text(constant: &GraphConstant) -> anyhow::Result<String> {
    if let Some(snapshot) = &constant.tabular {
        let columns = snapshot
            .columns()
            .iter()
            .map(|column| {
                Ok(format!(
                    "{}: {}",
                    serde_json::to_string(column.name().as_str())?,
                    serde_json::to_string(column.values())?
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        return Ok(format!("{{\n{}\n}}", columns.join(",\n")));
    }
    if constant.data_value == DataValue::Null {
        return Ok(initial_value(&constant.data_type).into());
    }
    if let DataValue::String(value) = &constant.data_value {
        return Ok(value.to_string());
    }
    if matches!(
        constant.data_type,
        ValueType::DataFrame | ValueType::DataSeries(_)
    ) {
        return Ok(initial_value(&constant.data_type).into());
    }
    crate::constant_values::json_text(&constant.data_value)
}
