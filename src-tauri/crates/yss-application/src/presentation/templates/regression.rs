use std::collections::BTreeMap;
use yss_sci_contract::regression::summary::LinearSummaryOptions;
use yss_ui_contract::{InvalidUiSpec, UiComponent, UiElement, UiSpec};

fn entries(
    options: &LinearSummaryOptions,
) -> Vec<(bool, &'static str, Option<&'static str>, bool, UiComponent)> {
    vec![
        (
            options.equation,
            "equation",
            Some("Equation"),
            false,
            UiComponent::Equation {
                binding: "equation".into(),
            },
        ),
        (
            options.model_summary,
            "modelSummary",
            Some("Model Summary"),
            false,
            UiComponent::KeyValue {
                binding: "summary".into(),
            },
        ),
        (
            options.anova,
            "anova",
            Some("ANOVA"),
            false,
            UiComponent::Table {
                binding: "anova".into(),
            },
        ),
        (
            options.coefficient_table,
            "coefficientTable",
            Some("Coefficients"),
            false,
            UiComponent::CoefficientTable {
                binding: "coefficients".into(),
            },
        ),
        (
            options.coefficient_chart,
            "coefficientMagnitude",
            Some("Coefficient Magnitude"),
            false,
            UiComponent::Chart {
                binding: "coefficientMagnitude".into(),
            },
        ),
        (
            options.hypothesis_test,
            "hypothesisTest",
            None,
            false,
            UiComponent::Analysis {
                binding: "hypothesis".into(),
            },
        ),
        (
            options.diagnostics,
            "diagnostics",
            Some("Diagnostics"),
            false,
            UiComponent::StatCard {
                binding: "conditionNumber".into(),
            },
        ),
        (
            options.residual_plot,
            "residualPlot",
            Some("Residuals vs Fitted"),
            true,
            UiComponent::Chart {
                binding: "residualPlot".into(),
            },
        ),
        (
            options.observations,
            "observations",
            Some("Fitted values and residuals"),
            true,
            UiComponent::Table {
                binding: "observations".into(),
            },
        ),
        (
            options.acf_pacf,
            "acfPacf",
            None,
            false,
            UiComponent::Analysis {
                binding: "acfPacf".into(),
            },
        ),
        (
            options.serial_tests,
            "serialTests",
            None,
            false,
            UiComponent::Analysis {
                binding: "serialTests".into(),
            },
        ),
    ]
}

pub(in crate::presentation) fn spec_for(options: &LinearSummaryOptions) -> UiSpec {
    let mut elements = BTreeMap::new();
    let mut children = Vec::new();
    for (enabled, id, title, collapsible, component) in entries(options) {
        if !enabled {
            continue;
        }
        children.push(id.into());
        let content_id = if let Some(title) = title {
            let content_id = format!("{id}-content");
            elements.insert(
                id.into(),
                UiElement {
                    component: UiComponent::Section {
                        title: title.into(),
                        collapsible,
                    },
                    visible: true,
                    children: vec![content_id.clone()],
                },
            );
            content_id
        } else {
            id.into()
        };
        elements.insert(
            content_id,
            UiElement {
                component,
                visible: true,
                children: vec![],
            },
        );
    }
    elements.insert(
        "report".into(),
        UiElement {
            component: UiComponent::Column { gap: 4 },
            visible: true,
            children,
        },
    );
    UiSpec {
        root: "report".into(),
        elements,
    }
}

pub(in crate::presentation) fn validate_bindings(
    spec: &UiSpec,
    allowed: &UiSpec,
) -> Result<(), InvalidUiSpec> {
    // The default template grants only bindings supported by this result's summary options.
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
    fn template_wire_and_binding_types_match_the_report_contract() {
        let mut spec = spec_for(&LinearSummaryOptions {
            equation: true,
            model_summary: true,
            anova: true,
            coefficient_table: true,
            coefficient_chart: true,
            hypothesis_test: true,
            diagnostics: true,
            residual_plot: true,
            observations: true,
            acf_pacf: true,
            serial_tests: true,
            ..Default::default()
        });
        spec.validate().unwrap();
        let allowed = spec.clone();
        validate_bindings(&spec, &allowed).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../src/tests/fixtures/node-system-contracts/regression-ui.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(&spec).unwrap(), fixture);
        spec.elements.get_mut("anova-content").unwrap().component = UiComponent::Chart {
            binding: "anova".into(),
        };
        assert!(validate_bindings(&spec, &allowed).is_err());
        spec.elements.get_mut("anova-content").unwrap().component = UiComponent::Table {
            binding: "missing".into(),
        };
        assert!(validate_bindings(&spec, &allowed).is_err());
    }
}
