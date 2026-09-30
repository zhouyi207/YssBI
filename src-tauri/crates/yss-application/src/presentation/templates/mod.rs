pub(super) mod regression;

use yss_ui_contract::{InvalidUiSpec, UiComponent, UiElement, UiSpec};

pub(super) fn structured() -> UiSpec {
    UiSpec {
        root: "report".into(),
        elements: [
            (
                "report".into(),
                UiElement {
                    component: UiComponent::Column { gap: 4 },
                    visible: true,
                    children: vec!["result".into()],
                },
            ),
            (
                "result".into(),
                UiElement {
                    component: UiComponent::Structured {
                        binding: "result".into(),
                    },
                    visible: true,
                    children: vec![],
                },
            ),
        ]
        .into(),
    }
}

pub(super) fn validate_bindings(spec: &UiSpec, allowed: &UiSpec) -> Result<(), InvalidUiSpec> {
    for element in spec.elements.values() {
        if let Some(binding) = element.component.binding()
            && !allowed
                .elements
                .values()
                .any(|element| element.component.binding() == Some(binding))
        {
            return Err(InvalidUiSpec);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_page_binds_the_result_without_granting_native_report_queries() {
        let allowed = structured();
        allowed.validate().unwrap();
        validate_bindings(&allowed, &allowed).unwrap();
        assert_eq!(
            serde_json::to_value(&allowed).unwrap()["elements"]["result"]["component"],
            serde_json::json!({"type": "structured", "props": {"binding": "result"}})
        );
        let mut invalid = allowed.clone();
        invalid.elements.get_mut("result").unwrap().component = UiComponent::Table {
            binding: "observations".into(),
        };
        invalid.validate().unwrap();
        assert!(validate_bindings(&invalid, &allowed).is_err());
        invalid.elements.get_mut("result").unwrap().component = UiComponent::KeyValue {
            binding: "result".into(),
        };
        assert!(validate_bindings(&invalid, &allowed).is_err());
    }
}
