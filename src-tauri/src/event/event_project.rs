use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphProjectionReplacementDto {
    pub graph_path: String,
    pub projection: crate::node_system::analysis::EditorGraphProjectionDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMutationResultDto {
    pub deltas: Vec<crate::node_system::document::ResourceDeltaEvent>,
    pub projection_replacements: Vec<GraphProjectionReplacementDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventProject {
    #[serde(rename_all = "camelCase")]
    ProjectLoaded {
        path: Option<String>,
    },
    ProjectCleared,
    #[serde(rename_all = "camelCase")]
    GraphDelta {
        delta: crate::node_system::document::GraphDeltaEvent<
            crate::node_system::document::GraphDocumentPatch,
        >,
    },
    #[serde(rename_all = "camelCase")]
    ResourceMutationCommitted {
        result: ResourceMutationResultDto,
    },
    #[serde(rename_all = "camelCase")]
    ProjectSaved {
        path: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_mutation_result_uses_explicit_atomic_wire_fields() {
        let result = ResourceMutationResultDto {
            deltas: Vec::new(),
            projection_replacements: Vec::new(),
        };

        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "deltas": [],
                "projectionReplacements": [],
            })
        );
    }
}
