//! Durable persistence and authoritative publication of runtime variable effects.

use super::*;

pub(in crate::project) fn install_variable_effect_snapshots(
    data: &mut ProjectData,
    transaction: &ProjectHistoryTransaction,
    undo: bool,
) -> Result<Vec<yss_variable_contract::VariableId>, String> {
    let snapshots = transaction
        .variable_effect_snapshots
        .as_ref()
        .ok_or_else(|| "durable variable-effect history is missing snapshots".to_string())?;
    let selected = if undo {
        &snapshots.before
    } else {
        &snapshots.after
    };
    let mut ids = Vec::with_capacity(selected.len());
    for (key, snapshot) in selected {
        let id = key
            .0
            .strip_prefix("variables/")
            .ok_or_else(|| format!("invalid variable history resource '{}'", key.0))
            .and_then(|value| uuid::Uuid::parse_str(value).map_err(|error| error.to_string()))
            .map(yss_variable_contract::VariableId::from)?;
        match snapshot {
            Some(snapshot) => {
                let variable: yss_variable_contract::VariableInstance =
                    serde_json::from_value(snapshot.clone()).map_err(|error| error.to_string())?;
                if variable.id != id {
                    return Err(format!(
                        "variable history snapshot does not match resource '{}'",
                        key.0
                    ));
                }
                data.variables.insert(id, variable);
            }
            None => {
                data.variables.remove(&id);
            }
        }
        ids.push(id);
    }
    Ok(ids)
}

pub(in crate::project) fn variable_scope_graph_path(
    scope: &yss_variable_contract::VariableScope,
) -> Result<Option<GraphResourcePath>, String> {
    match scope {
        yss_variable_contract::VariableScope::Global => Ok(None),
        yss_variable_contract::VariableScope::Event { event_path }
        | yss_variable_contract::VariableScope::Function {
            function_path: event_path,
        } => GraphResourcePath::new(event_path)
            .map(Some)
            .map_err(|error| error.to_string()),
    }
}

pub(in crate::project) fn variable_history_scope(
    data: &ProjectData,
    transaction: &ProjectHistoryTransaction,
    id: yss_variable_contract::VariableId,
    undo: bool,
) -> Result<yss_variable_contract::VariableScope, String> {
    if let Some(variable) = data.variables.get(&id) {
        return Ok(variable.scope.clone());
    }
    let snapshots = transaction
        .variable_effect_snapshots
        .as_ref()
        .ok_or_else(|| "durable variable-effect history is missing snapshots".to_string())?;
    let opposite = if undo {
        &snapshots.after
    } else {
        &snapshots.before
    };
    let key = yss_project_history::VariableResourceKey(format!("variables/{id}").into());
    let snapshot = opposite
        .get(&key)
        .and_then(Option::as_ref)
        .ok_or_else(|| format!("variable history cannot recover scope for '{id}'"))?;
    let variable: yss_variable_contract::VariableInstance =
        serde_json::from_value(snapshot.clone()).map_err(|error| error.to_string())?;
    if variable.id != id {
        return Err(format!(
            "variable history snapshot does not match resource 'variables/{id}'"
        ));
    }
    Ok(variable.scope)
}

pub(in crate::project) fn variable_effect_filesystem_mutations(
    data: &ProjectData,
    ids: &[yss_variable_contract::VariableId],
    transaction: &ProjectHistoryTransaction,
    undo: bool,
) -> Result<Vec<StagedFilesystemMutation>, String> {
    let mut writes_globals = false;
    let mut local_graph_paths = std::collections::BTreeSet::new();
    for id in ids {
        let scope = variable_history_scope(data, transaction, *id, undo)?;
        match variable_scope_graph_path(&scope)? {
            Some(path) => {
                local_graph_paths.insert(path);
            }
            None => writes_globals = true,
        }
    }

    let mut mutations = Vec::new();
    if writes_globals {
        let variables = data
            .variables
            .iter()
            .filter(|(_, variable)| {
                matches!(variable.scope, yss_variable_contract::VariableScope::Global)
            })
            .map(|(id, variable)| (*id, variable.clone()))
            .collect();
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: yss_project_layout::GLOBAL_VARIABLES_FILE.into(),
            contents: serde_json::to_vec_pretty(
                &crate::project::project_io::GlobalVariablesDocument {
                    schema_version: crate::project::project_io::SCHEMA_VERSION,
                    variables,
                },
            )
            .map_err(|error| error.to_string())?,
        });
    }
    for graph_path in local_graph_paths {
        let graph = data
            .graphs
            .get(&graph_path)
            .ok_or_else(|| format!("local variable graph '{graph_path}' is not loaded"))?;
        let local_variables = data
            .variables
            .iter()
            .filter(|(_, variable)| variable_scope_matches_graph(&variable.scope, &graph_path))
            .map(|(id, variable)| (*id, variable.clone()))
            .collect();
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: graph_path.as_str().into(),
            contents: crate::project::project_io::serialize_graph_resource_document(
                graph,
                local_variables,
            )
            .map_err(|error| error.to_string())?,
        });
    }
    Ok(mutations)
}

pub(in crate::project) fn validate_variable_effect_document(
    path: &std::path::Path,
    contents: &[u8],
) -> Result<(), String> {
    if path == std::path::Path::new(yss_project_layout::GLOBAL_VARIABLES_FILE) {
        serde_json::from_slice::<crate::project::project_io::GlobalVariablesDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn variable_scope_matches_graph(
    scope: &yss_variable_contract::VariableScope,
    graph_path: &GraphResourcePath,
) -> bool {
    match scope {
        yss_variable_contract::VariableScope::Event { event_path } => {
            event_path == graph_path.as_str()
        }
        yss_variable_contract::VariableScope::Function { function_path } => {
            function_path == graph_path.as_str()
        }
        yss_variable_contract::VariableScope::Global => false,
    }
}
