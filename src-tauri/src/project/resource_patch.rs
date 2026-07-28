use crate::project::{GraphResourceDocument, GraphResourcePath, WorksheetDocument};
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
        id: String,
        document: WorksheetDocument,
    },
    RemoveWorksheet {
        id: String,
    },
}
