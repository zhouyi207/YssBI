use crate::project::ProjectData;
use crate::schema::InvalidReferenceDTO;
use std::collections::{HashMap, HashSet};

/// Scan loaded graph nodes for references to missing variables, databases, or subgraphs.
pub fn collect_invalid_graph_references(
    data: &ProjectData,
) -> HashMap<String, Vec<InvalidReferenceDTO>> {
    let valid_variable_ids: HashSet<String> =
        data.variables.keys().map(|k| k.to_string()).collect();
    let valid_dataframe_ids: HashSet<String> = data.databases.keys().cloned().collect();
    let valid_graph_paths: HashSet<String> =
        data.graphs.keys().map(|path| path.as_str().to_string()).collect();

    let mut invalid_references = HashMap::new();

    for (graph_path, graph) in data.graphs.iter() {
        let data_state = graph.data_state.read().unwrap();
        let mut refs = Vec::new();

        for node in data_state.nodes.values() {
            let mut inv = InvalidReferenceDTO {
                node_id: node.id.to_string(),
                variable_id: None,
                dataframe_id: None,
                sub_graph_path: None,
            };
            let mut has_invalid = false;

            if let Some(vid) = node.instance_params.variable_id() {
                if !valid_variable_ids.contains(vid) {
                    inv.variable_id = Some(vid.to_string());
                    has_invalid = true;
                }
            }
            if let Some(dfid) = node.instance_params.dataframe_id() {
                if !valid_dataframe_ids.contains(dfid) {
                    inv.dataframe_id = Some(dfid.to_string());
                    has_invalid = true;
                }
            }
            if let Some(sub_graph_path) = node.instance_params.sub_graph_path() {
                let normalized = crate::project::normalize_graph_resource_path(sub_graph_path);
                if !valid_graph_paths.contains(&normalized) {
                    inv.sub_graph_path = Some(sub_graph_path.to_string());
                    has_invalid = true;
                }
            }

            if has_invalid {
                refs.push(inv);
            }
        }

        if !refs.is_empty() {
            invalid_references.insert(graph_path.as_str().to_string(), refs);
        }
    }

    invalid_references
}
