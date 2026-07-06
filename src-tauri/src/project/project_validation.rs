use crate::graph::GraphId;
use crate::project::ProjectData;
use crate::schema::InvalidReferenceDTO;
use std::collections::{HashMap, HashSet};

/// Scan loaded graph nodes for references to missing variables, databases, or subgraphs.
pub fn collect_invalid_graph_references(
    data: &ProjectData,
) -> HashMap<GraphId, Vec<InvalidReferenceDTO>> {
    let valid_variable_ids: HashSet<String> = data.variables.keys().map(|k| k.to_string()).collect();
    let valid_dataframe_ids: HashSet<String> = data.databases.keys().cloned().collect();
    let valid_graph_ids: HashSet<GraphId> = data.graphs.keys().copied().collect();

    let mut invalid_references = HashMap::new();

    for (graph_id, graph) in data.graphs.iter() {
        let data_state = graph.data_state.read().unwrap();
        let mut refs = Vec::new();

        for node in data_state.nodes.values() {
            let mut inv = InvalidReferenceDTO {
                node_id: node.id.to_string(),
                variable_id: None,
                dataframe_id: None,
                sub_graph_id: None,
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
            if let Some(sgid) = node.instance_params.sub_graph_id() {
                let parsed = uuid::Uuid::parse_str(sgid).ok().map(GraphId::from);
                if let Some(gid) = parsed {
                    if !valid_graph_ids.contains(&gid) {
                        inv.sub_graph_id = Some(sgid.to_string());
                        has_invalid = true;
                    }
                } else {
                    inv.sub_graph_id = Some(sgid.to_string());
                    has_invalid = true;
                }
            }

            if has_invalid {
                refs.push(inv);
            }
        }

        if !refs.is_empty() {
            invalid_references.insert(*graph_id, refs);
        }
    }

    invalid_references
}
