//! Display-only bindings into one authoritative structured result.
use crate::graph::results::structured::{at_path, project};
use serde::Deserialize;
use std::collections::BTreeMap;
use yss_data_contract::TabularScalar;
use yss_node_kernel::RuntimeValue;
use yss_ui_contract::{InvalidUiSpec, UiComponent, UiElement, UiSpec};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Display {
    sections: BTreeMap<String, Section>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Section {
    title: String,
    kind: Kind,
    path: String,
    #[serde(default)]
    columns: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Table,
    Equation,
    Stability,
}

pub(in crate::presentation) fn spec_for(value: &RuntimeValue) -> Result<UiSpec, InvalidUiSpec> {
    let RuntimeValue::Record(fields) = value.unannotated() else {
        return Err(InvalidUiSpec);
    };
    let Some(metadata) = fields.get("report_display") else {
        return Ok(super::structured());
    };
    // Reuse the result projection budget before deserializing display metadata.
    let bounded = project(metadata).map_err(|_| InvalidUiSpec)?;
    let display: Display = serde_json::from_value(
        crate::result_encoding::runtime_value_to_json(&bounded).map_err(|_| InvalidUiSpec)?,
    )
    .map_err(|_| InvalidUiSpec)?;
    if display.sections.len() > 24 {
        return Err(InvalidUiSpec);
    }
    let mut spec = super::structured();
    let mut children = Vec::new();
    for (id, section) in display.sections {
        if id.is_empty()
            || id.len() > 48
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
            || section.title.len() > 200
            || section.columns.len() > 32
            || section
                .columns
                .iter()
                .any(|(key, label)| key.len() > 100 || label.len() > 100)
        {
            return Err(InvalidUiSpec);
        }
        let target = at_path(value, &section.path).ok_or(InvalidUiSpec)?;
        let binding = format!("report_{id}");
        let component = match (section.kind, target) {
            (Kind::Table, RuntimeValue::List(rows)) if !section.columns.is_empty() => {
                if let Some(first) = rows.first() {
                    let RuntimeValue::Record(fields) = first.unannotated() else {
                        return Err(InvalidUiSpec);
                    };
                    if section.columns.keys().any(|key| {
                        !matches!(
                            fields.get(key.as_str()).map(RuntimeValue::unannotated),
                            Some(RuntimeValue::Scalar(_))
                        )
                    }) {
                        return Err(InvalidUiSpec);
                    }
                }
                UiComponent::Table { binding }
            }
            (Kind::Stability, RuntimeValue::List(_)) => UiComponent::Chart { binding },
            (Kind::Equation, RuntimeValue::Scalar(TabularScalar::String(text)))
                if text.len() <= 16_384 =>
            {
                UiComponent::Equation { binding }
            }
            _ => return Err(InvalidUiSpec),
        };
        let section_id = format!("report-{id}");
        let content_id = format!("{section_id}-content");
        children.push(section_id.clone());
        spec.elements.insert(
            section_id,
            UiElement {
                component: UiComponent::Section {
                    title: section.title,
                    collapsible: true,
                },
                visible: true,
                children: vec![content_id.clone()],
            },
        );
        spec.elements.insert(
            content_id,
            UiElement {
                component,
                visible: true,
                children: vec![],
            },
        );
    }
    spec.elements.insert(
        "raw-data".into(),
        UiElement {
            component: UiComponent::Section {
                title: "Structured result".into(),
                collapsible: true,
            },
            visible: true,
            children: vec!["result".into()],
        },
    );
    children.push("raw-data".into());
    spec.elements.get_mut("report").unwrap().children = children;
    spec.validate()?;
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn runtime(value: serde_json::Value) -> RuntimeValue {
        match value {
            serde_json::Value::Object(fields) => RuntimeValue::Record(Arc::new(
                fields
                    .into_iter()
                    .map(|(k, v)| (k.into(), runtime(v)))
                    .collect(),
            )),
            serde_json::Value::Array(values) => {
                RuntimeValue::List(values.into_iter().map(runtime).collect::<Vec<_>>().into())
            }
            serde_json::Value::String(value) => TabularScalar::String(value.into()).into(),
            serde_json::Value::Number(value) => {
                RuntimeValue::float64(value.as_f64().unwrap()).unwrap()
            }
            _ => TabularScalar::Null.into(),
        }
    }
    #[test]
    fn report_metadata_grants_only_existing_typed_result_bindings() {
        let mut value = serde_json::json!({
            "coefficients": [{"variable":"x", "estimate":1}], "equation":"y = 1 x",
            "stability": [{"re":0.5,"im":0,"modulus":0.5}],
            "report_display":{"sections":{
                "coefficients":{"kind":"table","title":"Coefficients","path":"/coefficients","columns":{"variable":"Variable","estimate":"Estimate"}},
                "equation":{"kind":"equation","title":"Equation","path":"/equation"},
                "roots":{"kind":"stability","title":"Stability","path":"/stability"}
            }}
        });
        let spec = spec_for(&runtime(value.clone())).unwrap();
        spec.validate().unwrap();
        assert!(
            matches!(&spec.elements["report-coefficients-content"].component,UiComponent::Table{binding} if binding=="report_coefficients")
        );
        value["report_display"]["sections"]["coefficients"]["path"] = "/missing".into();
        assert!(spec_for(&runtime(value.clone())).is_err());
        value["report_display"]["sections"]["coefficients"]["path"] = "/equation".into();
        assert!(spec_for(&runtime(value)).is_err());
    }
}
