//! Graph diagnostics and execution failures use the shared desktop locale and translations.
use yss_application::graph::run::ExecutionApplicationError;
use yss_graph_editor::projection::{
    EditorDiagnosticModel, EditorDiagnosticSeverity, EditorNodeModel, EditorProjectionModel,
};
use yss_ui::inspector::{InlineMessage, Severity};
use yss_ui::text::{format, locale, translate};

pub(crate) fn diagnostic_message(diagnostic: &EditorDiagnosticModel) -> InlineMessage {
    let severity = match diagnostic.severity {
        EditorDiagnosticSeverity::Error => Severity::Error,
        EditorDiagnosticSeverity::Warning => Severity::Warning,
        EditorDiagnosticSeverity::Information => Severity::Information,
    };
    InlineMessage::new(severity, graph_diagnostic(diagnostic))
}

pub fn graph_diagnostic(diagnostic: &EditorDiagnosticModel) -> String {
    let fallback = || {
        let code = diagnostic.code.as_ref();
        if code.len() <= 128
            && code.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && code.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.".contains(&byte)
            })
        {
            format("native.text.diagnosticCode", &[("code", code.to_owned())])
        } else {
            translate("native.text.diagnosticFallback")
        }
    };
    yss_graph_diagnostics::render_diagnostic(
        locale(),
        &diagnostic.code,
        &diagnostic.message_key,
        &diagnostic.arguments,
    )
    .unwrap_or_else(fallback)
}

pub(crate) fn diagnostic_description(
    diagnostic: &EditorDiagnosticModel,
    projection: &EditorProjectionModel,
    owner: Option<&EditorNodeModel>,
) -> String {
    let arguments = diagnostic
        .arguments
        .iter()
        .map(|(key, value)| {
            let label = crate::problems::location::argument_label(
                key,
                value,
                &diagnostic.location,
                projection,
                owner,
            );
            (
                key.clone(),
                label.map_or_else(|| value.clone(), String::into_boxed_str),
            )
        })
        .collect();
    yss_graph_diagnostics::render_diagnostic(
        locale(),
        &diagnostic.code,
        &diagnostic.message_key,
        &arguments,
    )
    .unwrap_or_else(|| graph_diagnostic(diagnostic))
}

pub fn run_failure_key(code: yss_graph_execution::error::RunFailureCode) -> &'static str {
    use yss_graph_execution::error::RunFailureCode;
    match code {
        RunFailureCode::KernelFailed => "kernelFailed",
        RunFailureCode::KernelNotFound => "kernelNotFound",
        RunFailureCode::InvalidNumericInput => "invalidNumericInput",
        RunFailureCode::ShapeMismatch => "shapeMismatch",
        RunFailureCode::GroupSchemaMismatch => "groupSchemaMismatch",
        RunFailureCode::GroupKeyCollision => "groupKeyCollision",
        RunFailureCode::InvalidParameter => "invalidParameter",
        RunFailureCode::UnalignedSeries => "unalignedSeries",
        RunFailureCode::BudgetExceeded => "budgetExceeded",
        RunFailureCode::InputLayoutMismatch => "inputLayoutMismatch",
        RunFailureCode::OutputContractMismatch => "outputContractMismatch",
        RunFailureCode::ScientificFailure => "scientificFailure",
        RunFailureCode::DivisionByZero => "divisionByZero",
        RunFailureCode::NonFiniteResult => "nonFiniteResult",
        RunFailureCode::DeadlineExceeded => "deadlineExceeded",
        RunFailureCode::ResourceUnavailable => "resourceUnavailable",
        RunFailureCode::InputResultUnavailable => "inputResultUnavailable",
        RunFailureCode::FinalizationFailed => "finalizationFailed",
    }
}

pub fn run_failure(code: yss_graph_execution::error::RunFailureCode) -> String {
    translate(&format!("runFailure.causes.{}", run_failure_key(code)))
}

/// Explains a run that Application refused before it started.
pub fn run_rejection(error: &anyhow::Error) -> String {
    let cause = match error.downcast_ref::<ExecutionApplicationError>() {
        Some(ExecutionApplicationError::DraftChanged) => "graph_draft_changed",
        Some(ExecutionApplicationError::GraphNotReady) => "graph_not_ready",
        Some(ExecutionApplicationError::GraphResolutionFailed { .. }) => "graph_resolution_failed",
        Some(ExecutionApplicationError::GraphPlan(_)) => "graph_plan_failed",
        Some(
            ExecutionApplicationError::SessionCapture(_)
            | ExecutionApplicationError::StaleSession(_),
        ) => "stale_project_lifecycle",
        Some(ExecutionApplicationError::Cancelled) => {
            return yss_ui::text::t("native.canvas.runCancelled").into();
        }
        Some(ExecutionApplicationError::DeadlineExceeded) => "deadlineExceeded",
        _ => return yss_ui::text::t("native.canvas.runRequestFailed").into(),
    };
    yss_ui::text::translate(&format!("runFailure.causes.{cause}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_analysis_contract::DiagnosticLocation;
    use yss_graph_document::{NodeId, NodePosition, PortAddress};
    use yss_graph_editor::projection::*;
    use yss_node_protocol::PortDirection;

    #[test]
    fn friendly_port_diagnostic_preserves_canonical_facts_and_literal_labels() {
        let node_id = NodeId::from_bytes([1; 16]);
        let address = PortAddress::declared(node_id, "left".parse().expect("port key"));
        let diagnostic = EditorDiagnosticModel {
            code: "graph.input.unbound".into(),
            message_key: "diagnostics.graph.input.unbound".into(),
            severity: EditorDiagnosticSeverity::Warning,
            blocking: true,
            arguments: [("port".into(), address.to_string().into())].into(),
            location: DiagnosticLocation::Port(address.clone()),
            related: Box::new([]),
        };
        let node = EditorNodeModel {
            node_id,
            node_type: "test.math.add".parse().expect("node type"),
            position: NodePosition { x: 0.0, y: 0.0 },
            display: EditorNodeDisplay {
                title: "Addition".into(),
                user_label: Some("Total %{port}".into()),
                icon_id: None,
                style_id: None,
            },
            ports: Box::new([EditorPortModel {
                address,
                display: EditorPortDisplay {
                    label: "Left".into(),
                    instance_label: Some("First %{input}".into()),
                },
                direction: PortDirection::Input,
                orphan: false,
                can_remove: false,
                connections: EditorPortConnectionCapabilities {
                    current: 0,
                    maximum: Some(1),
                    can_append: true,
                    can_replace: false,
                    can_move: false,
                },
                input: None,
                accepted_type: "Numeric".into(),
                type_state: EditorPortTypeState::Exact {
                    display: "Numeric".into(),
                    data_type: None,
                },
                resolved_schema: None,
                status: EditorPortStatus::Resolved,
            }]),
            port_instance_additions: Box::new([]),
            parameter_groups: Box::new([]),
            capabilities: EditorNodeCapabilities { managed: false },
            diagnostics: Box::new([diagnostic.clone()]),
        };
        let graph_path = yss_graph_document::GraphResourcePath::new("events/test.yssbi-event")
            .expect("graph path");
        let projection = EditorProjectionModel {
            basis: EditorProjectionBasis {
                graph_path: graph_path.clone(),
                registry_fingerprint: [0; 32],
                semantic_input_hash: [0; 32],
            },
            graph_path,
            nodes: Box::new([node]),
            connections: Box::new([]),
            diagnostics: Box::new([diagnostic.clone()]),
            outcome: EditorResolutionOutcome::Incomplete,
        };
        let message = diagnostic_description(&diagnostic, &projection, None);
        assert!(message.contains("Total %{port} · First %{input}"));
        assert!(!message.contains(&node_id.to_string()));
        assert_eq!(projection.diagnostics[0], diagnostic);
        assert!(graph_diagnostic(&diagnostic).contains(&node_id.to_string()));
    }
}
