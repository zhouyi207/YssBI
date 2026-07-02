use super::ProjectState;
use super::unique_name;
use crate::event::InferredPinType;
use crate::graph::GraphId;
use crate::graph::pin::PinKind;
use crate::graph::value::{DataType, DataValue};
use crate::schema::{data_type_to_container, data_type_to_pin_type};
use crate::variable::VariableId;
use crate::variable::{VariableInstance, VariableScope};

#[derive(Debug, Clone)]
pub struct VariableReferenceSync {
    pub graph_id: GraphId,
    pub pin_types: Vec<InferredPinType>,
}

impl ProjectState {
    pub fn add_variable(
        &self,
        name: &str,
        data_type: DataType,
        data_value: DataValue,
        description: &str,
        scope: VariableScope,
        tags: Vec<String>,
    ) -> VariableInstance {
        let unique_var_name = {
            let project_data = self.project_data.read().unwrap();
            let existing: Vec<&str> = project_data
                .variables
                .values()
                .map(|v| v.name.as_str())
                .collect();
            unique_name::unique_name(name, existing)
        };

        let variable_instance = VariableInstance {
            id: VariableId::new(),
            name: unique_var_name,
            data_type: data_type,
            data_value: data_value,
            description: description.to_string(),
            scope: scope,
            tags: tags,
        };

        self.project_data
            .write()
            .unwrap()
            .variables
            .insert(variable_instance.id, variable_instance.clone());
        variable_instance
    }

    pub fn remove_variable(&self, variable_id: &VariableId) -> Option<VariableInstance> {
        self.project_data
            .write()
            .unwrap()
            .variables
            .remove(&variable_id)
    }

    pub fn get_variable(&self, variable_id: &VariableId) -> Option<VariableInstance> {
        self.project_data
            .read()
            .unwrap()
            .variables
            .get(variable_id)
            .cloned()
    }

    /// 更新变量（部分字段），返回更新后的实例
    pub fn update_variable(
        &self,
        variable_id: &VariableId,
        name: Option<String>,
        data_type: Option<DataType>,
        data_value: Option<DataValue>,
        description: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Option<VariableInstance> {
        let mut data = self.project_data.write().unwrap();
        let var = data.variables.get_mut(variable_id)?;
        if let Some(n) = name {
            var.name = n;
        }
        if let Some(dt) = data_type {
            let changed = var.data_type != dt;
            var.data_type = dt;
            if changed && data_value.is_none() {
                var.data_value = var.data_type.default_value();
            }
        }
        if let Some(dv) = data_value {
            var.data_value = dv;
        }
        if let Some(d) = description {
            var.description = d;
        }
        if let Some(t) = tags {
            var.tags = t;
        }
        Some(var.clone())
    }

    pub fn sync_variable_references(
        &self,
        variable_id: &VariableId,
        name_changed: bool,
        type_changed: bool,
        updated: &VariableInstance,
    ) -> Vec<VariableReferenceSync> {
        if !name_changed && !type_changed {
            return Vec::new();
        }

        let var_id_str = variable_id.to_string();
        let new_data_type = &updated.data_type;
        let new_name = &updated.name;
        let project_data = self.project_data.read().unwrap();
        let mut syncs = Vec::new();

        for (graph_id, graph) in project_data.graphs.iter() {
            let data_state = graph.data_state.read().unwrap();
            let mut inferred_pins = Vec::new();

            if type_changed {
                for node in data_state.nodes.values() {
                    if node.instance_params.variable_id() != Some(var_id_str.as_str()) {
                        continue;
                    }
                    for &pin_id in &node.pin_ids {
                        if let Some(pin) = data_state.pins.get(&pin_id) {
                            if pin.definition.kind == PinKind::Data {
                                inferred_pins.push(InferredPinType {
                                    pin_id,
                                    pin_type: data_type_to_pin_type(new_data_type).to_string(),
                                    container_type: data_type_to_container(new_data_type)
                                        .map(|s| s.to_string()),
                                    type_display: Some(new_data_type.to_string()),
                                    data_type: Some(new_data_type.clone()),
                                });
                            }
                        }
                    }
                }
            }

            let nodes_to_update: Vec<_> = data_state
                .nodes
                .values()
                .filter(|n| n.instance_params.variable_id() == Some(var_id_str.as_str()))
                .map(|n| n.id)
                .collect();

            drop(data_state);

            if nodes_to_update.is_empty() && inferred_pins.is_empty() {
                continue;
            }

            {
                let mut data_state = graph.data_state.write().unwrap();
                for ipt in &inferred_pins {
                    data_state
                        .pin_types
                        .insert(ipt.pin_id, new_data_type.clone());
                }
                for nid in &nodes_to_update {
                    let pin_ids = if let Some(node) = data_state.nodes.get(nid) {
                        node.pin_ids.clone()
                    } else {
                        Vec::new()
                    };

                    if name_changed {
                        for pin_id in pin_ids {
                            if let Some(pin) = data_state.pins.get_mut(&pin_id) {
                                if pin.definition.kind == PinKind::Data {
                                    pin.definition.name = new_name.clone();
                                }
                            }
                        }
                    }
                }
            }

            if !inferred_pins.is_empty() {
                syncs.push(VariableReferenceSync {
                    graph_id: *graph_id,
                    pin_types: inferred_pins,
                });
            }
        }

        syncs
    }

    pub fn apply_global_variables_from_disk(&self, project_path: &str) -> Result<(), String> {
        let root = super::project_root_from_path(project_path);
        let entries =
            super::read_global_variable_index_entries(root.as_path()).map_err(|e| e.to_string())?;
        let mut data = self.project_data.write().unwrap();
        data.variables
            .retain(|_, variable| !matches!(variable.scope, VariableScope::Global));
        for entry in entries {
            let id = uuid::Uuid::parse_str(&entry.id)
                .map_err(|e| format!("Invalid variable id '{}': {}", entry.id, e))?;
            let variable_id = VariableId::from(id);
            data.variables.insert(
                variable_id,
                VariableInstance {
                    id: variable_id,
                    name: entry.name,
                    data_type: entry.data_type,
                    data_value: entry.data_value,
                    description: entry.description,
                    scope: entry.scope,
                    tags: entry.tags,
                },
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_int_variable(state: &ProjectState) -> VariableInstance {
        state.add_variable(
            "x",
            DataType::Int64,
            DataValue::Int64(42),
            "",
            VariableScope::Global,
            vec![],
        )
    }

    #[test]
    fn update_variable_resets_value_to_type_default_when_type_changes_without_value() {
        let state = ProjectState::new();
        let variable = add_int_variable(&state);

        let updated = state
            .update_variable(
                &variable.id,
                None,
                Some(DataType::Boolean),
                None,
                None,
                None,
            )
            .expect("updated variable");

        assert_eq!(updated.data_type, DataType::Boolean);
        assert_eq!(updated.data_value, DataValue::Boolean(false));
    }

    #[test]
    fn update_variable_keeps_explicit_value_when_type_and_value_are_both_changed() {
        let state = ProjectState::new();
        let variable = add_int_variable(&state);

        let updated = state
            .update_variable(
                &variable.id,
                None,
                Some(DataType::Boolean),
                Some(DataValue::Boolean(true)),
                None,
                None,
            )
            .expect("updated variable");

        assert_eq!(updated.data_type, DataType::Boolean);
        assert_eq!(updated.data_value, DataValue::Boolean(true));
    }
}
