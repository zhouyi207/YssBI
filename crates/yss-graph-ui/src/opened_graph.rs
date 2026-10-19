//! Native graph read model; mutation authority stays in Application/Project.
use anyhow::{Result, anyhow};
use std::sync::Arc;
use yss_application::graph::{
    editing::GraphEditResponse, open::OpenGraphApplicationReceipt, results::GraphResultState,
};
use yss_graph_editor::projection::EditorProjectionModel;
use yss_project::GraphEditingState;
use yss_project_identity::ProjectInstanceId;
pub struct OpenedGraph {
    pub(crate) project: ProjectInstanceId,
    pub(crate) projection: Arc<EditorProjectionModel>,
    pub(crate) editing: GraphEditingState,
    pub(crate) results: GraphResultState,
    pub(crate) language: String,
    dependencies: yss_graph_resource_contract::GraphDependencyManifest,
}

impl OpenedGraph {
    pub fn project(&self) -> &ProjectInstanceId {
        &self.project
    }
    pub fn projection(&self) -> &Arc<EditorProjectionModel> {
        &self.projection
    }
    pub fn editing(&self) -> &GraphEditingState {
        &self.editing
    }
    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn from_open(receipt: OpenGraphApplicationReceipt, language: &str) -> Self {
        Self {
            project: receipt.project_instance_id().clone(),
            language: language.to_owned(),
            editing: receipt.editing().clone(),
            results: receipt.result_state().clone(),
            dependencies: receipt
                .analysis()
                .semantic_snapshot()
                .dependencies()
                .clone(),
            projection: Arc::new(receipt.into_projection()),
        }
    }

    pub(crate) fn install_edit(
        &mut self,
        response: GraphEditResponse,
        language: &str,
    ) -> Result<()> {
        if response.update.projection_replacement.projection.graph_path
            != self.projection.graph_path
        {
            return Err(anyhow!(yss_ui::text::t("native.project.wrongResource")));
        }
        self.language = language.to_owned();
        self.projection = Arc::new(response.update.projection_replacement.projection);
        self.editing = response.editing;
        self.dependencies = response.update.dependencies;
        self.install_results(response.result_state);
        Ok(())
    }

    pub(crate) fn install_results(&mut self, state: GraphResultState) {
        if self.results.execution_session_id == state.execution_session_id
            && self.results.semantic_input_hash == state.semantic_input_hash
            && self.results.revision > state.revision
        {
            return;
        }
        self.results = state;
    }

    /// Uses the resolver's observed dependencies, including unresolved references.
    pub fn depends_on_resource(&self, resource: &yss_project_history::ResourceKey) -> bool {
        use yss_graph_resource_contract::GraphDependencyKey;
        use yss_project_history::ResourceKey;
        self.dependencies
            .entries()
            .keys()
            .any(|dependency| match (dependency, resource) {
                (GraphDependencyKey::Database(id), ResourceKey::Database(key)) => id == &key.0,
                (
                    GraphDependencyKey::Function(id) | GraphDependencyKey::FunctionBody(id),
                    ResourceKey::Function(key),
                ) => id == &key.0,
                (
                    GraphDependencyKey::Function(id) | GraphDependencyKey::FunctionBody(id),
                    ResourceKey::Graph(path),
                ) => id.as_ref() == path.as_str(),
                _ => false,
            })
    }

    pub(crate) fn same_projection(&self, graph: &Self) -> bool {
        self.project == graph.project
            && self.editing.version == graph.editing.version
            && self.language == graph.language
            && self.results.execution_session_id == graph.results.execution_session_id
            && self.dependencies == graph.dependencies
            && self.projection == graph.projection
    }

    pub(crate) fn replace(&mut self, mut graph: Self) {
        if self.results.execution_session_id == graph.results.execution_session_id
            && self.results.semantic_input_hash == graph.results.semantic_input_hash
            && self.results.revision > graph.results.revision
        {
            graph.results = self.results.clone();
        }
        *self = graph;
    }
}
