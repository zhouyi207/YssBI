use crate::graph_document::GraphResourcePath;
use crate::project::{GraphResourceDocument, WorksheetDocument, WorksheetResourcePath};
use crate::variable::{VariableId, VariableInstance};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub enum ResourceDocumentPatch {
    InsertGraph {
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    },
    /// Publish an on-disk graph declaration without installing a resident document.
    DeclareGraph {
        path: GraphResourcePath,
        revision: crate::project::ResourceRevision,
    },
    RemoveGraph {
        path: GraphResourcePath,
        revision: crate::project::ResourceRevision,
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
        revision: crate::project::ResourceRevision,
    },
    MoveWorksheet {
        from: WorksheetResourcePath,
        to: WorksheetResourcePath,
        moved: WorksheetDocument,
    },
}
