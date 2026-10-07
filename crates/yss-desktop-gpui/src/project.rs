//! Native project and editor read models; mutation authority stays in Application/Project.
use std::sync::Arc;

use anyhow::{Result, anyhow};
use yss_application::{
    activity_panel::ActivityPanelDocument,
    graph::{
        editing::GraphEditResponse, open::OpenGraphApplicationReceipt, results::GraphResultState,
    },
    project::query::ProjectIndexSnapshot,
};
use yss_graph_editor::projection::EditorProjectionModel;
use yss_project::GraphEditingState;
use yss_project_identity::ProjectInstanceId;

pub struct DesktopProject {
    pub identity: ProjectInstanceId,
    pub index: Arc<yss_project::ProjectIndex>,
    pub panels: Vec<Arc<ActivityPanelDocument>>,
}

impl DesktopProject {
    pub fn new(identity: ProjectInstanceId, snapshot: ProjectIndexSnapshot) -> Self {
        Self {
            identity,
            index: Arc::new(snapshot.index),
            panels: snapshot.activity_panels.into_iter().map(Arc::new).collect(),
        }
    }
    pub fn first_graph(&self) -> Option<String> {
        self.index
            .event_graphs
            .first()
            .map(|graph| graph.path.clone())
            .or_else(|| {
                self.index
                    .function_graphs
                    .first()
                    .map(|graph| graph.path.clone())
            })
    }
}

pub struct OpenedGraph {
    pub project: ProjectInstanceId,
    pub projection: Arc<EditorProjectionModel>,
    pub editing: GraphEditingState,
    pub results: GraphResultState,
}

impl OpenedGraph {
    pub fn from_open(receipt: OpenGraphApplicationReceipt) -> Self {
        Self {
            project: receipt.project_instance_id().clone(),
            projection: Arc::new(receipt.projection().clone()),
            editing: receipt.editing().clone(),
            results: receipt.result_state().clone(),
        }
    }

    pub fn install_edit(&mut self, response: GraphEditResponse) -> Result<()> {
        if response.update.projection_replacement.projection.graph_path
            != self.projection.graph_path
        {
            return Err(anyhow!("图编辑回执属于另一个资源"));
        }
        self.projection = Arc::new(response.update.projection_replacement.projection);
        self.editing = response.editing;
        self.install_results(response.result_state);
        Ok(())
    }

    pub fn install_results(&mut self, state: GraphResultState) {
        if self.results.execution_session_id == state.execution_session_id
            && self.results.semantic_input_hash == state.semantic_input_hash
            && self.results.revision > state.revision
        {
            return;
        }
        self.results = state;
    }

    pub fn replace(&mut self, mut graph: Self) {
        if self.results.execution_session_id == graph.results.execution_session_id
            && self.results.semantic_input_hash == graph.results.semantic_input_hash
            && self.results.revision > graph.results.revision
        {
            graph.results = self.results.clone();
        }
        *self = graph;
    }
}
