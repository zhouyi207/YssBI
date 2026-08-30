use std::collections::{BTreeMap, BTreeSet};
use yss_graph_document::GraphResourcePath;
use yss_project_model::GraphResourceDocument;
use yss_variable_contract::{VariableId, VariableInstance};
use yss_worksheet_document::{WorksheetDocument, WorksheetResourcePath};

#[derive(Clone, Debug)]
pub enum ResourceDocumentPatch {
    InsertGraph {
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    },
    /// Publish an on-disk graph declaration without installing a resident document.
    DeclareGraph {
        path: GraphResourcePath,
        revision: yss_project_identity::ResourceRevision,
    },
    RemoveGraph {
        path: GraphResourcePath,
        revision: yss_project_identity::ResourceRevision,
    },
    UnloadGraph {
        path: GraphResourcePath,
    },
    MoveGraph {
        from: GraphResourcePath,
        to: GraphResourcePath,
        moved_before: GraphResourceDocument,
        moved: GraphResourceDocument,
        referenced_graphs_before: BTreeMap<GraphResourcePath, GraphResourceDocument>,
        referenced_graphs: BTreeMap<GraphResourcePath, GraphResourceDocument>,
        loaded_referenced_graphs: BTreeSet<GraphResourcePath>,
        referenced_variables_before: BTreeMap<VariableId, VariableInstance>,
        referenced_variables: BTreeMap<VariableId, VariableInstance>,
    },
    PatchVariables {
        updates: BTreeMap<VariableId, VariableInstance>,
        removals: BTreeSet<VariableId>,
    },
    UpsertWorksheet {
        path: WorksheetResourcePath,
        document: WorksheetDocument,
    },
    RemoveWorksheet {
        path: WorksheetResourcePath,
        revision: yss_project_identity::ResourceRevision,
    },
    MoveWorksheet {
        from: WorksheetResourcePath,
        to: WorksheetResourcePath,
        moved: WorksheetDocument,
    },
}
