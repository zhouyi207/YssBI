//! Readiness and diagnostic pages use Execution's closure and Analysis's location rules.
use super::*;
use yss_graph_analysis::GraphAnalysis;
use yss_graph_execution::{
    graph_preparation::GraphExecutionScope,
    plan::{NodeExecutionMode, PlanExecutionDemand, PlanNodeId},
};

pub(super) fn inspect(
    request: ValidateGraphRequest,
    path: &GraphResourcePath,
    analysis: &GraphAnalysis,
    hash: String,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    let semantics = analysis.semantic_snapshot();
    let mut nodes = BTreeSet::new();
    for id in &request.node_ids {
        let demand = PlanExecutionDemand::Node {
            node: PlanNodeId::from_existing(parse_node_id(id)?.to_string().into()),
            mode: NodeExecutionMode::Dependencies,
        };
        nodes.extend(
            GraphExecutionScope::select(path, semantics, &demand)
                .map_err(|_| invalid_edit_identity("nodeIds"))?
                .nodes(),
        );
    }
    let (ready, scope_node_count, diagnostics) = if request.node_ids.is_empty() {
        (
            semantics.ready().is_some(),
            semantics.nodes().len(),
            semantics.diagnostics().iter().collect::<Vec<_>>(),
        )
    } else {
        (
            semantics.nodes_ready(&nodes),
            nodes.len(),
            semantics.diagnostics_for_nodes(&nodes).collect::<Vec<_>>(),
        )
    };
    let offset = request.offset.min(diagnostics.len());
    let page = InspectionPage::known(
        offset,
        request.limit.min(diagnostics.len() - offset),
        diagnostics.len(),
    );
    let diagnostics = diagnostics
        .into_iter()
        .skip(offset)
        .take(request.limit)
        .map(|diagnostic| GraphDiagnosticInspection {
            code: diagnostic.code.as_str().into(),
            message_key: diagnostic.message_key.to_string(),
            blocking: diagnostic.blocking,
            severity: format!("{:?}", diagnostic.severity).to_lowercase(),
            location: format!("{:?}", diagnostic.primary),
            arguments: diagnostic
                .arguments
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        })
        .collect();
    Ok(AutomationCapabilityResult::GraphValidation(
        GraphValidation {
            graph_path: path.as_str().into(),
            graph_hash: hash,
            ready,
            node_ids: request.node_ids,
            scope_node_count,
            page,
            diagnostics,
        },
    ))
}
