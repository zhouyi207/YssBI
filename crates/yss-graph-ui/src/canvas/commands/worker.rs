//! One UI command commits its captured drafts and mutation as one authoritative edit.
use super::GraphCommand;
use crate::canvas::ports::input::commit::PortEdit;
use yss_application::{
    graph::editing::{GraphEditRequest, GraphEditResponse},
    session::ApplicationState,
};
use yss_graph_document::NodeId;
use yss_graph_editor::EditorGraphMutation;
use yss_node_protocol::ParameterValues;
use yss_project_identity::OperationId;

pub(super) struct ParameterEdit {
    pub(super) node_id: NodeId,
    pub(super) values: ParameterValues,
}

#[derive(Default)]
pub struct GraphCommandOutcome {
    pub(super) inputs: Vec<PortEdit>,
    pub(super) parameters: Vec<ParameterEdit>,
    pub(super) language: String,
    pub(super) applied_drafts: bool,
    pub(super) applied_operations: Vec<OperationId>,
    pub(super) response: Option<GraphEditResponse>,
    pub(super) error: Option<anyhow::Error>,
}

pub(super) struct GraphCommandRequest {
    pub(super) request: GraphEditRequest,
    pub(super) inputs: Vec<PortEdit>,
    pub(super) parameters: Vec<ParameterEdit>,
}

/// A save carries captured drafts, not the payload of an unrelated edit command.
pub struct GraphSaveRequest(pub(super) GraphCommandRequest);

impl GraphSaveRequest {
    pub fn commit(self, application: &ApplicationState) -> GraphCommandOutcome {
        self.0.commit(application, GraphCommand::Save)
    }
}

impl GraphCommandRequest {
    pub(super) fn commit(
        self,
        application: &ApplicationState,
        command: GraphCommand,
    ) -> GraphCommandOutcome {
        let mut outcome = GraphCommandOutcome {
            inputs: self.inputs,
            parameters: self.parameters,
            language: self.request.locale.clone(),
            ..Default::default()
        };
        if let Err(error) = apply(application, self.request, command, &mut outcome) {
            outcome.error = Some(error);
        }
        outcome
    }
}

fn apply(
    application: &ApplicationState,
    mut request: GraphEditRequest,
    command: GraphCommand,
    outcome: &mut GraphCommandOutcome,
) -> anyhow::Result<()> {
    let mut mutations = Vec::with_capacity(outcome.inputs.len() + outcome.parameters.len() + 1);
    mutations.extend(
        outcome
            .inputs
            .iter()
            .map(|input| EditorGraphMutation::SetLiteral {
                address: input.address.clone(),
                literal: Some(input.value.clone()),
            }),
    );
    mutations.extend(
        outcome
            .parameters
            .iter()
            .map(|edit| EditorGraphMutation::SetParameters {
                node_id: edit.node_id,
                parameters: edit.values.clone(),
            }),
    );
    let after = match command {
        GraphCommand::Save | GraphCommand::Undo | GraphCommand::Redo => Some(command),
        GraphCommand::CommitPortInputs
        | GraphCommand::RunAfterInputs(_)
        | GraphCommand::SetParameter { .. } => None,
        GraphCommand::Edit(mutation) => {
            mutations.push(mutation);
            None
        }
        GraphCommand::Paste { source, anchor } => {
            let snapshot = yss_graph_editor::deserialize_clipboard_subgraph(source.as_bytes())?;
            mutations.push(EditorGraphMutation::InsertSubgraph { snapshot, anchor });
            None
        }
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
            mutations.push(EditorGraphMutation::SetConstant {
                id,
                constant: Some(constant),
            });
            None
        }
    };
    if !mutations.is_empty() {
        let response = application.edit_graph_batch(request.clone(), mutations)?;
        request.version = response.editing.version;
        outcome.applied_operations.push(response.operation_id);
        request.operation_id = OperationId::new();
        outcome.applied_drafts = true;
        outcome.response = Some(response);
    }
    if let Some(command) = after {
        let response = match command {
            GraphCommand::Save => application.save_current_graph(request)?.graph,
            GraphCommand::Undo => application.change_graph_history(request, false)?,
            GraphCommand::Redo => application.change_graph_history(request, true)?,
            _ => unreachable!("only persistence and history commands follow a draft batch"),
        };
        outcome.applied_operations.push(response.operation_id);
        outcome.response = Some(response);
    }
    Ok(())
}
