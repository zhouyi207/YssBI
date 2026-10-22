//! Read-only graph facts for project-wide panels; reading never makes a graph resident.
use std::sync::Arc;

use thiserror::Error;
use yss_graph_document::{GraphResourceKind, GraphResourcePath};
use yss_graph_editor::projection::{
    EditorProjectionInput, EditorProjectionModel, build_editor_projection,
};
use yss_project_identity::ProjectInstanceId;

use super::catalog::{
    ProjectCatalogReadError, localized_project_facts_from_index, map_project_catalog_error,
};
use super::inputs::{GraphInputError, GraphResolutionContext, graph_result_inputs};
use super::results::{GraphResultState, observe_graph_result_inputs};
use crate::session::{ApplicationSession, ApplicationState, SessionCaptureError};

#[derive(Clone, Debug)]
pub struct GraphOverview {
    pub graph_path: GraphResourcePath,
    pub name: String,
    pub projection: Arc<EditorProjectionModel>,
    /// Function bodies run inside their callers and have no results of their own.
    pub results: Option<GraphResultState>,
}

#[derive(Clone, Debug, Default)]
pub struct GraphOverviews {
    /// Every graph in the project index when the overview was read.
    pub project_graphs: Vec<GraphResourcePath>,
    pub graphs: Vec<GraphOverview>,
    /// Listed graphs whose documents could not be read or resolved.
    pub unavailable: Vec<GraphResourcePath>,
}

#[derive(Debug, Error)]
pub enum GraphOverviewError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("graph overview belongs to another project instance")]
    ProjectIdentityMismatch,
    #[error("captured graph overview session changed")]
    SessionChanged,
    #[error("project graphs could not be listed")]
    Project(#[source] ProjectCatalogReadError),
    #[error(transparent)]
    Input(#[from] GraphInputError),
}

#[derive(Debug, Error)]
enum UnavailableGraph {
    #[error("graph editing queue is busy")]
    Busy,
    #[error("graph document could not be read")]
    Read(#[source] yss_project::ProjectOperationError),
    #[error("graph document is invalid")]
    Document(#[source] yss_graph_runtime::GraphMaterializationError),
    #[error("graph dependencies could not be mapped")]
    Input(#[source] GraphInputError),
    #[error("graph projection failed")]
    Projection(#[source] yss_graph_editor::projection::EditorProjectionError),
}

impl ApplicationState {
    /// Resolves `graphs`, or every graph in the project index when `None`.
    pub fn graph_overviews(
        &self,
        project: &ProjectInstanceId,
        graphs: Option<&[GraphResourcePath]>,
        locale: &str,
    ) -> Result<GraphOverviews, GraphOverviewError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project {
            return Err(GraphOverviewError::ProjectIdentityMismatch);
        }
        let index = captured
            .project()
            .read_project_index(project)
            .map_err(|error| GraphOverviewError::Project(map_project_catalog_error(error)))?;
        let facts = localized_project_facts_from_index(&captured, &index)
            .map_err(GraphOverviewError::Project)?;
        let catalog = GraphResolutionContext::from_project_facts(&captured, facts)?;
        let names = index
            .event_graphs
            .iter()
            .map(|graph| (&graph.path, &graph.name))
            .chain(
                index
                    .function_graphs
                    .iter()
                    .map(|graph| (&graph.path, &graph.name)),
            )
            .filter_map(|(path, name)| Some((GraphResourcePath::new(path.clone()).ok()?, name)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let project_graphs = names.keys().cloned().collect::<Vec<_>>();
        let requested = graphs.map_or(project_graphs.as_slice(), |graphs| graphs);
        let mut overview = GraphOverviews::default();
        for (path, name) in requested
            .iter()
            .filter_map(|path| Some((path, names.get(path)?.as_str())))
        {
            match read_graph(&captured, &catalog, path, name, locale) {
                Ok(graph) => overview.graphs.push(graph),
                Err(error) => {
                    // A replaced project or catalog makes every remaining read stale.
                    catalog.revalidate(&captured)?;
                    self.revalidate_captured_session(&captured)
                        .map_err(|_| GraphOverviewError::SessionChanged)?;
                    tracing::debug!(
                        code = "graph_overview_unavailable",
                        %error,
                        "Graph overview skipped an unreadable graph"
                    );
                    overview.unavailable.push(path.clone());
                }
            }
        }
        self.revalidate_captured_session(&captured)
            .map_err(|_| GraphOverviewError::SessionChanged)?;
        overview.project_graphs = project_graphs;
        Ok(overview)
    }

    /// Current result summaries without resolving documents again. A graph whose
    /// observed inputs no longer have the given hash is omitted.
    pub fn graph_result_states(
        &self,
        project: &ProjectInstanceId,
        graphs: &[(GraphResourcePath, [u8; 32])],
    ) -> Result<Vec<(GraphResourcePath, GraphResultState)>, GraphOverviewError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project {
            return Err(GraphOverviewError::ProjectIdentityMismatch);
        }
        let states = graphs
            .iter()
            .filter_map(|(path, semantic_input_hash)| {
                let cache = captured
                    .execution()
                    .query_result_cache_states(path.as_str(), semantic_input_hash)?;
                Some((
                    path.clone(),
                    GraphResultState {
                        revision: cache.revision,
                        execution_session_id: captured.execution_session_id(),
                        semantic_input_hash: *semantic_input_hash,
                        outputs: cache.outputs,
                        connections: cache.connections,
                    },
                ))
            })
            .collect();
        self.revalidate_captured_session(&captured)
            .map_err(|_| GraphOverviewError::SessionChanged)?;
        Ok(states)
    }
}

fn read_graph(
    captured: &ApplicationSession,
    catalog: &GraphResolutionContext,
    path: &GraphResourcePath,
    name: &str,
    locale: &str,
) -> Result<GraphOverview, UnavailableGraph> {
    // Observing result inputs must not interleave with an edit of the same graph.
    let _permit = captured
        .coordinate_graph_edit(path)
        .map_err(|_| UnavailableGraph::Busy)?;
    let resource = captured
        .project()
        .read_graph_resource_snapshot(captured.project_instance_id(), path)
        .map_err(UnavailableGraph::Read)?;
    let document = captured
        .graph()
        .materialize_open_candidate(&resource.document)
        .map_err(UnavailableGraph::Document)?;
    let mut context = catalog.clone();
    context
        .include_functions(captured, &document)
        .map_err(UnavailableGraph::Input)?;
    let analysis = context.resolve(captured, path, &document, locale);
    let projection = build_editor_projection(EditorProjectionInput {
        graph_path: path,
        document: &document,
        analysis: &analysis,
        registry_fingerprint: context.registry_fingerprint,
    })
    .map_err(UnavailableGraph::Projection)?;
    context
        .revalidate(captured)
        .map_err(UnavailableGraph::Input)?;
    let results = (path.kind() == GraphResourceKind::EventGraph).then(|| {
        observe_graph_result_inputs(
            captured,
            path.as_str(),
            graph_result_inputs(
                path,
                &analysis,
                &context.database,
                context.registry_fingerprint,
            ),
        )
    });
    Ok(GraphOverview {
        graph_path: path.clone(),
        name: name.to_owned(),
        projection: Arc::new(projection),
        results,
    })
}

#[cfg(test)]
mod tests;
