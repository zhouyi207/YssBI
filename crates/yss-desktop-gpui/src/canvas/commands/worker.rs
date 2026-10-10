//! Sequential UI intents reuse the original typed use cases and their exact returned versions.
use super::GraphCommand;
use crate::canvas::ports::input::commit::PortEdit;
use yss_application::{
    graph::editing::{GraphEditRequest, GraphEditResponse},
    session::ApplicationState,
};
use yss_graph_editor::EditorGraphMutation;
use yss_project_identity::OperationId;

#[derive(Default)]
pub(crate) struct GraphCommandOutcome {
    pub(super) inputs: Vec<PortEdit>,
    pub(super) language: String,
    pub(super) applied_inputs: usize,
    pub(super) response: Option<GraphEditResponse>,
    pub(super) error: Option<anyhow::Error>,
}

pub(crate) struct GraphCommandRequest {
    pub(super) request: GraphEditRequest,
    pub(super) inputs: Vec<PortEdit>,
    pub(super) command: GraphCommand,
}

impl GraphCommandRequest {
    pub(crate) fn commit(self, application: &ApplicationState) -> GraphCommandOutcome {
        apply(application, self.request, self.inputs, self.command)
    }
}

fn apply(
    application: &ApplicationState,
    mut request: GraphEditRequest,
    inputs: Vec<PortEdit>,
    command: GraphCommand,
) -> GraphCommandOutcome {
    let mut outcome = GraphCommandOutcome {
        inputs,
        language: request.locale.clone(),
        ..Default::default()
    };
    for input in &outcome.inputs {
        match application.edit_graph(
            request.clone(),
            EditorGraphMutation::SetLiteral {
                address: input.address.clone(),
                literal: Some(input.value.clone()),
            },
        ) {
            Ok(response) => {
                request.version = response.editing.version;
                request.operation_id = OperationId::new();
                outcome.applied_inputs += 1;
                outcome.response = Some(response);
            }
            Err(error) => {
                outcome.error = Some(error.into());
                return outcome;
            }
        }
    }
    if !matches!(
        command,
        GraphCommand::CommitPortInputs | GraphCommand::RunAfterPortInputs(_)
    ) {
        match apply_command(application, request, command) {
            Ok(response) => outcome.response = Some(response),
            Err(error) => outcome.error = Some(error),
        }
    }
    outcome
}

fn apply_command(
    application: &ApplicationState,
    request: GraphEditRequest,
    command: GraphCommand,
) -> anyhow::Result<GraphEditResponse> {
    Ok(match command {
        GraphCommand::CommitPortInputs | GraphCommand::RunAfterPortInputs(_) => {
            unreachable!("input-only command handled before applying a graph action")
        }
        GraphCommand::Paste { source, anchor } => {
            let snapshot = yss_graph_editor::deserialize_clipboard_subgraph(source.as_bytes())?;
            application.edit_graph(
                request,
                EditorGraphMutation::InsertSubgraph { snapshot, anchor },
            )?
        }
        GraphCommand::Edit(mutation) => application.edit_graph(request, mutation)?,
        GraphCommand::Undo => application.change_graph_history(request, false)?,
        GraphCommand::Redo => application.change_graph_history(request, true)?,
        GraphCommand::Save => application.save_current_graph(request)?.graph,
        GraphCommand::UpdateConstant {
            id,
            name,
            data_type,
            value,
        } => {
            let document = application.current_graph_document(
                &request.project_instance_id,
                &request.graph_path,
                request.version,
            )?;
            let mut constant = document
                .constants
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("constant disappeared"))?
                .clone();
            constant.name = name;
            constant.data_type = data_type;
            if let Some(value) = value {
                constant.data_value =
                    crate::canvas::authoring::parse_constant_input(value, &constant.data_type)?;
                constant.tabular = None;
            }
            application.edit_graph(
                request,
                EditorGraphMutation::SetConstant {
                    id,
                    constant: Some(constant),
                },
            )?
        }
        GraphCommand::SetParameter {
            node_id,
            key,
            value,
        } => application.edit_graph(
            request,
            EditorGraphMutation::SetParameters {
                node_id,
                parameters: [(key, value)].into_iter().collect(),
            },
        )?,
    })
}
