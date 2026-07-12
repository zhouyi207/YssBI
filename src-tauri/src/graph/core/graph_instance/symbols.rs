use super::*;

impl GraphInstance {
    pub fn resolve_variable_nodes(&self, variables: &HashMap<String, (String, DataType)>) {
        let mut data_state = self.data_state.write().unwrap();
        let variable_nodes: Vec<_> = data_state
            .nodes
            .values()
            .filter_map(|node| {
                node.instance_params
                    .variable_id()
                    .and_then(|variable_id| variables.get(variable_id))
                    .map(|(name, data_type)| (node.id, name.clone(), data_type.clone()))
            })
            .collect();

        for (node_id, variable_name, data_type) in variable_nodes {
            let Some(node) = data_state.nodes.get(&node_id) else {
                continue;
            };
            let pin_ids = node.pin_ids.clone();
            for pin_id in pin_ids {
                let Some(pin) = data_state.pins.get_mut(&pin_id) else {
                    continue;
                };
                if pin.definition.kind != PinKind::Data {
                    continue;
                }
                pin.definition.name = variable_name.clone();
                data_state.pin_types.insert(pin_id, data_type.clone());
            }
        }
    }

    pub fn resolve_dataframe_nodes(&self, dataframes: &HashMap<String, String>) {
        let mut data_state = self.data_state.write().unwrap();
        let dataframe_nodes: Vec<_> = data_state
            .nodes
            .values()
            .filter(|node| node.definition.node_type == "Data:Get DataFrame")
            .filter_map(|node| {
                node.instance_params
                    .dataframe_id()
                    .and_then(|dataframe_id| dataframes.get(dataframe_id))
                    .map(|name| (node.id, name.clone()))
            })
            .collect();

        for (node_id, dataframe_label) in dataframe_nodes {
            let Some(node) = data_state.nodes.get(&node_id) else {
                continue;
            };
            let pin_ids = node.pin_ids.clone();
            for pin_id in pin_ids {
                let Some(pin) = data_state.pins.get_mut(&pin_id) else {
                    continue;
                };
                if pin.definition.kind == PinKind::Data {
                    pin.definition.name = dataframe_label.clone();
                    pin.definition.data_type =
                        Some(PinDataTypeDefinition::concrete(DataType::DataFrame));
                    data_state.pin_types.insert(pin_id, DataType::DataFrame);
                }
            }
        }
    }
}

impl std::fmt::Debug for GraphInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphInstance")
            .field("resource_path", &self.resource_path)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("function_inputs", &self.function_inputs)
            .field("function_outputs", &self.function_outputs)
            .field("data_state", &self.data_state)
            .finish_non_exhaustive()
    }
}
