use super::ProjectState;
use crate::graph::value::DataType;
use crate::graph::{GraphId, GraphInstance, GraphRecompileResult, GraphRecompileScope};
use std::collections::HashMap;
use std::sync::Arc;

/// Mutable graph access within a project write lock, with scoped symbol tables.
pub struct GraphMutContext<'a> {
    graph: &'a mut GraphInstance,
    pub variable_symbols: HashMap<String, (String, DataType)>,
    pub dataframe_symbols: HashMap<String, String>,
}

impl<'a> GraphMutContext<'a> {
    pub fn graph(&mut self) -> &mut GraphInstance {
        self.graph
    }

    pub fn graph_ref(&self) -> &GraphInstance {
        self.graph
    }

    /// Refresh variable/dataframe bindings on nodes after topology or catalog changes.
    pub fn sync_runtime_symbols(&mut self) {
        ProjectState::apply_runtime_symbols(
            self.graph,
            &self.variable_symbols,
            &self.dataframe_symbols,
        );
    }

    pub fn recompile(&self, scope: GraphRecompileScope) -> GraphRecompileResult {
        self.graph.recompile(scope)
    }
}

impl ProjectState {
    /// Bind registry and schema provider onto a graph (no symbol resolve / recompile).
    pub(crate) fn bind_graph_runtime(graph: &mut GraphInstance, state: &ProjectState) {
        let registry = {
            let store = state.project_store.read().unwrap();
            Arc::clone(&store.node_register)
        };
        graph.set_registry(registry);
        graph.set_schema_provider(state.build_schema_provider());
    }

    /// Acquire project write lock, bind runtime context, and run a graph mutation closure.
    pub fn with_graph_mut<R>(
        &self,
        graph_id: &GraphId,
        f: impl FnOnce(GraphMutContext<'_>) -> Result<R, String>,
    ) -> Result<R, String> {
        let (variable_symbols, dataframe_symbols) = {
            let data = self.project_data.read().unwrap();
            let graph = data
                .graphs
                .get(graph_id)
                .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
            (
                Self::variable_symbols_from_variables(
                    &data.variables,
                    graph_id,
                    &graph.kind,
                ),
                Self::dataframe_symbols_from_databases(&data.databases),
            )
        };

        let mut data = self.project_data.write().unwrap();
        let graph = data
            .graphs
            .get_mut(graph_id)
            .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

        Self::bind_graph_runtime(graph, self);

        f(GraphMutContext {
            graph,
            variable_symbols,
            dataframe_symbols,
        })
    }

    pub(crate) fn apply_runtime_symbols(
        graph: &mut GraphInstance,
        variable_symbols: &HashMap<String, (String, DataType)>,
        dataframe_symbols: &HashMap<String, String>,
    ) {
        graph.resolve_variable_nodes(variable_symbols);
        graph.resolve_dataframe_nodes(dataframe_symbols);
    }
}
