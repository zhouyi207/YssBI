//! One-time conversion of schema 3 variable resources to schema 4 graph constants.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;
use yss_data_contract::{DataType, DataValue};
use yss_graph_document::{ConstantId, GraphConstant, GraphResourceKind};
use yss_project_filesystem::{
    ProjectFilesystemError, StagedFilesystemMutation, read_secure_project_file,
};
use yss_project_layout::PROJECT_METADATA_FILE;

use crate::project_io::GraphResourceFile;

const LEGACY_VARIABLES_FILE: &str = "variables.yssbi-vars";

fn migration_error(error: impl std::fmt::Display) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: format!("constant migration: {error}"),
    }
}

pub(crate) fn prepare_constant_migration(
    root: &Path,
) -> Result<Vec<StagedFilesystemMutation>, ProjectFilesystemError> {
    let bytes = read_secure_project_file(root, Path::new(PROJECT_METADATA_FILE))
        .map_err(migration_error)?;
    let mut manifest: Value = serde_json::from_slice(&bytes).map_err(migration_error)?;
    if manifest["schemaVersion"] == crate::manifest::CURRENT_PROJECT_SCHEMA_VERSION {
        return Ok(Vec::new());
    }
    if manifest["schemaVersion"] != 3 {
        return Err(migration_error("only schema 3 projects can be converted"));
    }
    let legacy_file = match read_secure_project_file(root, Path::new(LEGACY_VARIABLES_FILE)) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(migration_error(error)),
    };
    let globals = match &legacy_file {
        Some(bytes) => {
            let value: Value = serde_json::from_slice(bytes).map_err(migration_error)?;
            decode_constants(value.get("variables").cloned())?
        }
        None => BTreeMap::new(),
    };
    let mut unused_globals = globals.keys().copied().collect::<BTreeSet<_>>();
    let index = crate::scan_graph_resource_index(root).map_err(migration_error)?;
    let mut graphs = Vec::new();
    for entry in index.entries() {
        let path = PathBuf::from(entry.path.as_str());
        let contents = read_secure_project_file(root, &path).map_err(migration_error)?;
        let mut raw: Value = serde_json::from_slice(&contents).map_err(migration_error)?;
        let local = decode_constants(
            raw.as_object_mut()
                .and_then(|raw| raw.remove("localVariables")),
        )?;
        let mut graph: GraphResourceFile = serde_json::from_value(raw).map_err(migration_error)?;
        if graph.kind != entry.kind {
            return Err(migration_error(format!(
                "graph '{}' has an inconsistent kind",
                entry.path
            )));
        }
        for constant in local.into_values() {
            insert_constant(&mut graph, constant)?;
        }
        let mut referenced = BTreeSet::new();
        let mut literals = Vec::new();
        for node in graph.document.nodes.values_mut() {
            let literal_type = match node.node_type.as_str() {
                "yssbi.constant.bool" => Some(DataType::Boolean),
                "yssbi.constant.int64" => Some(DataType::Int64),
                "yssbi.constant.float64" => Some(DataType::Float64),
                "yssbi.constant.string" => Some(DataType::String),
                _ => None,
            };
            if let Some(data_type) = literal_type {
                let value = node
                    .parameters
                    .remove(&"value".parse().map_err(migration_error)?);
                let data_value = match (value, &data_type) {
                    (None, _) => yss_graph_document::default_value_for(&data_type),
                    (Some(Value::Bool(value)), DataType::Boolean) => DataValue::Boolean(value),
                    (Some(Value::String(value)), DataType::String) => DataValue::String(value),
                    (Some(Value::Number(value)), DataType::Int64) => DataValue::Int64(
                        value
                            .as_i64()
                            .ok_or_else(|| migration_error("invalid Int64 literal"))?,
                    ),
                    (Some(Value::Number(value)), DataType::Float64) => DataValue::Float64(
                        value
                            .as_f64()
                            .ok_or_else(|| migration_error("invalid Float64 literal"))?,
                    ),
                    _ => return Err(migration_error("literal value does not match its type")),
                };
                let id = ConstantId::new();
                literals.push(GraphConstant {
                    id,
                    name: node.user_label.clone().unwrap_or_else(|| "Constant".into()),
                    data_type,
                    data_value,
                    tabular: None,
                    description: String::new(),
                    tags: vec![],
                });
                node.node_type = "yssbi.constant.get".parse().map_err(migration_error)?;
                node.parameters.insert(
                    "constant".parse().map_err(migration_error)?,
                    Value::String(id.to_string()),
                );
                continue;
            }
            if node.node_type.as_str() != "yssbi.project.variable.get" {
                continue;
            }
            let old_key = "variable".parse().map_err(migration_error)?;
            let id = node
                .parameters
                .remove(&old_key)
                .and_then(|value| value.as_str().map(str::to_owned))
                .and_then(|value| value.strip_prefix("variables/").map(str::to_owned))
                .ok_or_else(|| {
                    migration_error(format!(
                        "node '{}' has an invalid variable reference",
                        node.id
                    ))
                })?
                .parse::<ConstantId>()
                .map_err(migration_error)?;
            referenced.insert(id);
            node.node_type = "yssbi.constant.get".parse().map_err(migration_error)?;
            node.parameters.insert(
                "constant".parse().map_err(migration_error)?,
                Value::String(id.to_string()),
            );
        }
        for constant in literals {
            insert_constant(&mut graph, constant)?;
        }
        for id in referenced {
            if graph.document.constants.contains_key(&id) {
                continue;
            }
            if let Some(constant) = globals.get(&id) {
                insert_constant(&mut graph, constant.clone())?;
                unused_globals.remove(&id);
            }
            // A previously missing reference remains unresolved, with the same identity.
        }
        graphs.push((path, graph));
    }
    if !unused_globals.is_empty() {
        let owner = graphs
            .iter()
            .position(|(_, graph)| graph.kind == GraphResourceKind::Event);
        let owner = owner.unwrap_or_else(|| {
            graphs.push((
                PathBuf::from("events/Constants.yssbi-event"),
                GraphResourceFile {
                    kind: GraphResourceKind::Event,
                    name: "Constants".into(),
                    document: Default::default(),
                    function: None,
                },
            ));
            graphs.len() - 1
        });
        for id in unused_globals {
            insert_constant(&mut graphs[owner].1, globals[&id].clone())?;
        }
    }
    let mut mutations = Vec::new();
    for (relative_path, graph) in graphs {
        let contents = serde_json::to_vec_pretty(&graph).map_err(migration_error)?;
        crate::project_io::parse_graph_resource_document(&contents, &relative_path, graph.kind)
            .map_err(migration_error)?;
        mutations.push(StagedFilesystemMutation::Write {
            relative_path,
            contents,
        });
    }
    if legacy_file.is_some() {
        mutations.push(StagedFilesystemMutation::RemoveFile {
            relative_path: LEGACY_VARIABLES_FILE.into(),
        });
    }
    manifest["schemaVersion"] = crate::manifest::CURRENT_PROJECT_SCHEMA_VERSION.into();
    // Validate the manifest before any file is replaced.
    let _: crate::manifest::ProjectManifest =
        serde_json::from_value(manifest.clone()).map_err(migration_error)?;
    mutations.push(StagedFilesystemMutation::Write {
        relative_path: PROJECT_METADATA_FILE.into(),
        contents: serde_json::to_vec_pretty(&manifest).map_err(migration_error)?,
    });
    Ok(mutations)
}

fn decode_constants(
    value: Option<Value>,
) -> Result<BTreeMap<ConstantId, GraphConstant>, ProjectFilesystemError> {
    let mut constants: BTreeMap<ConstantId, GraphConstant> = value
        .map(serde_json::from_value)
        .transpose()
        .map_err(migration_error)?
        .unwrap_or_default();
    for (id, constant) in &mut constants {
        if id != &constant.id {
            return Err(migration_error(
                "constant map identity does not match its value",
            ));
        }
        // Legacy handles are local to their snapshots; they are never resolved through Project.
        let previous = format!("var:{id}");
        let current = yss_graph_document::constant_handle(id);
        match &mut constant.data_value {
            DataValue::DataFrame(handle) if *handle == previous => *handle = current,
            DataValue::DataSeries(series) if series.id == previous => series.id = current,
            _ => {}
        }
        yss_graph_document::normalize_constant_value(constant).map_err(migration_error)?;
    }
    Ok(constants)
}

fn insert_constant(
    graph: &mut GraphResourceFile,
    mut constant: GraphConstant,
) -> Result<(), ProjectFilesystemError> {
    if graph.document.constants.contains_key(&constant.id) {
        return Err(migration_error("duplicate constant identity"));
    }
    let base = if constant.name.trim().is_empty() {
        "Constant"
    } else {
        constant.name.trim()
    }
    .to_owned();
    constant.name = base.clone();
    let mut suffix = 2;
    while graph
        .document
        .constants
        .values()
        .any(|existing| existing.name == constant.name)
    {
        constant.name = format!("{base} {suffix}");
        suffix += 1;
    }
    graph.document.constants.insert(constant.id, constant);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use yss_project_model::{GraphResourceDocument, ProjectData};

    fn legacy_project() -> crate::fixtures::TempProject {
        let mut data = ProjectData::new();
        data.graphs.insert(
            "events/Main.yssbi-event".parse().unwrap(),
            GraphResourceDocument::new("Main", GraphResourceKind::Event),
        );
        let fixture = crate::fixtures::TempProject::activate("constant-migration", data);
        let session = fixture.state().capture_project_session().unwrap();
        let path = session.root.as_path().join(PROJECT_METADATA_FILE);
        let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        manifest["schemaVersion"] = json!(3);
        std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        fixture
    }

    #[test]
    fn activation_migrates_references_literals_and_unused_values_once() {
        let fixture = legacy_project();
        let session = fixture.state().capture_project_session().unwrap();
        let root = session.root.as_path();
        let local = ConstantId::new();
        let global = ConstantId::new();
        let unused = ConstantId::new();
        let scalar = |id: ConstantId, value: i64| json!({"id":id,"name":"Threshold","dataType":{"kind":"Int64"},"dataValue":{"Int64":value},"scope":{"kind":"Global"}});
        std::fs::write(root.join(LEGACY_VARIABLES_FILE), serde_json::to_vec(&json!({"variables":{
            global.to_string(): scalar(global, 42),
            unused.to_string(): {"id":unused,"name":"Table","dataType":{"kind":"DataFrame"},"dataValue":{"DataFrame":format!("var:{unused}")},"tabular":{"columns":{"x":[1,2]}}}
        }})).unwrap()).unwrap();
        let path = root.join("events/Main.yssbi-event");
        let mut raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        raw["localVariables"] = json!({local.to_string(): scalar(local, 7)});
        let mut graph: GraphResourceFile = serde_json::from_value(raw.clone()).unwrap();
        for id in [local, global] {
            let node = yss_graph_document::NodeId::new();
            graph.document.nodes.insert(
                node,
                yss_graph_document::DocumentNode {
                    id: node,
                    node_type: "yssbi.project.variable.get".parse().unwrap(),
                    position: yss_graph_document::NodePosition { x: 0.0, y: 0.0 },
                    parameters: [(
                        "variable".parse().unwrap(),
                        json!(format!("variables/{id}")),
                    )]
                    .into(),
                    user_label: None,
                },
            );
        }
        let literal = yss_graph_document::NodeId::new();
        graph.document.nodes.insert(
            literal,
            yss_graph_document::DocumentNode {
                id: literal,
                node_type: "yssbi.constant.float64".parse().unwrap(),
                position: yss_graph_document::NodePosition { x: 10.0, y: 20.0 },
                parameters: [("value".parse().unwrap(), json!(2.5))].into(),
                user_label: Some("Rate".into()),
            },
        );
        raw["document"] = serde_json::to_value(&graph.document).unwrap();
        std::fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
        crate::ProjectState::new()
            .prepare_project_activation(Some(root))
            .unwrap();
        assert!(!root.join(LEGACY_VARIABLES_FILE).exists());
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(raw.get("localVariables").is_none());
        let migrated: GraphResourceFile = serde_json::from_value(raw).unwrap();
        assert_eq!(migrated.document.constants.len(), 4);
        assert_eq!(
            migrated.document.constants[&global].data_value,
            DataValue::Int64(42)
        );
        assert_eq!(
            migrated.document.constants[&local].data_value,
            DataValue::Int64(7)
        );
        assert_eq!(
            migrated.document.constants[&unused].data_value,
            DataValue::DataFrame(format!("constant:{unused}"))
        );
        assert_eq!(
            migrated.document.constants[&unused]
                .tabular
                .as_ref()
                .unwrap()
                .row_count(),
            2
        );
        assert!(
            migrated
                .document
                .nodes
                .values()
                .all(|node| node.node_type.as_str() == "yssbi.constant.get")
        );
        assert_eq!(migrated.document.nodes[&literal].position.x, 10.0);
        assert!(prepare_constant_migration(root).unwrap().is_empty());
    }

    #[test]
    fn invalid_legacy_value_keeps_all_original_files() {
        let fixture = legacy_project();
        let session = fixture.state().capture_project_session().unwrap();
        let root = session.root.as_path();
        let id = ConstantId::new();
        let bytes = serde_json::to_vec(&json!({"variables":{id.to_string():{"id":id,"name":"Broken","dataType":{"kind":"Int64"},"dataValue":{"String":"wrong"}}}})).unwrap();
        std::fs::write(root.join(LEGACY_VARIABLES_FILE), &bytes).unwrap();
        let manifest = std::fs::read(root.join(PROJECT_METADATA_FILE)).unwrap();
        let graph = std::fs::read(root.join("events/Main.yssbi-event")).unwrap();
        assert!(
            crate::ProjectState::new()
                .prepare_project_activation(Some(root))
                .is_err()
        );
        assert_eq!(
            std::fs::read(root.join(LEGACY_VARIABLES_FILE)).unwrap(),
            bytes
        );
        assert_eq!(
            std::fs::read(root.join(PROJECT_METADATA_FILE)).unwrap(),
            manifest
        );
        assert_eq!(
            std::fs::read(root.join("events/Main.yssbi-event")).unwrap(),
            graph
        );
    }
}
