//! Format bounded, already-read values once while retaining their presentation kind.
use gpui_kit::SharedString;
use yss_data_contract::TabularScalar;
use yss_node_kernel::RuntimeValue;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Text,
    Number,
    Bool(bool),
    Null,
}

pub(super) struct Cell {
    pub text: SharedString,
    pub kind: Kind,
}

impl Cell {
    pub fn text(text: String) -> Self {
        Self {
            text: text.into(),
            kind: Kind::Text,
        }
    }

    pub fn new(value: &RuntimeValue, format: fn(&RuntimeValue) -> String) -> Self {
        let kind = match value.unannotated() {
            RuntimeValue::Scalar(TabularScalar::Null) => Kind::Null,
            RuntimeValue::Scalar(TabularScalar::Bool(checked)) => Kind::Bool(*checked),
            RuntimeValue::Scalar(
                TabularScalar::Integer(_) | TabularScalar::Unsigned(_) | TabularScalar::Float64(_),
            ) => Kind::Number,
            _ => Kind::Text,
        };
        let mut text = String::new();
        write(value, format, &mut text);
        Self {
            text: text.into(),
            kind,
        }
    }
}

// Lists in a sequence cell stay one cell. Do not infer columns from their contents.
fn write(value: &RuntimeValue, format: fn(&RuntimeValue) -> String, text: &mut String) {
    match value.unannotated() {
        RuntimeValue::List(values) => {
            text.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    text.push_str(", ");
                }
                write(value, format, text);
            }
            text.push(']');
        }
        RuntimeValue::Record(fields) => {
            text.push('{');
            for (index, (key, value)) in fields.iter().enumerate() {
                if index > 0 {
                    text.push_str(", ");
                }
                text.push_str(&serde_json::to_string(key).expect("string display"));
                text.push_str(": ");
                write(value, format, text);
            }
            text.push('}');
        }
        _ => text.push_str(&format(value)),
    }
}
