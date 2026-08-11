use crate::project::{
    GraphResourceDocument, GraphResourcePath, WorksheetDocument, WorksheetResourcePath,
};
use crate::variable::{VariableId, VariableInstance};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub enum ResourceDocumentPatch {
    InsertGraph {
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    },
    RemoveGraph {
        path: GraphResourcePath,
        revision: crate::node_system::document::ResourceRevision,
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
        revision: crate::node_system::document::ResourceRevision,
    },
    MoveWorksheet {
        from: WorksheetResourcePath,
        to: WorksheetResourcePath,
        moved: WorksheetDocument,
    },
}
