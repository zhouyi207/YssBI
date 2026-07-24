use super::*;

// 持久化格式（Phase B）
//
// 磁盘格式使用扁平的 `nodes[]`（pin 内联）+ 扁平的 `connections[]`。
// 静态 pin 的完整定义在加载后由 registry 经 `set_registry`
// 重新挂载；动态/可重复 pin 自带完整定义覆盖。运行期缓存
// （`pin_types` / `type_var_bindings` / `resolved_schema`）不落盘。
//
// Editor viewport (pan/zoom) is not document data; per-project view state lives in
// frontend editor view state memento.

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphDocSer<'a> {
    name: &'a str,
    kind: GraphKind,
    #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
    function_inputs: &'a [FunctionSignaturePin],
    #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
    function_outputs: &'a [FunctionSignaturePin],
    nodes: Vec<GraphNodeSer<'a>>,
    connections: Vec<Connection>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphNodeSer<'a> {
    id: NodeId,
    node_type: &'a str,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    type_var_map: &'a HashMap<TypeVarId, TypeVarDefinition>,
    position: NodePosition,
    instance_params: &'a NodeInstanceParams,
    pins: Vec<&'a PinInstance>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDocDe {
    name: String,
    kind: GraphKind,
    #[serde(default)]
    function_inputs: Vec<FunctionSignaturePin>,
    #[serde(default)]
    function_outputs: Vec<FunctionSignaturePin>,
    #[serde(default)]
    nodes: Vec<GraphNodeDe>,
    #[serde(default)]
    connections: Vec<Connection>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GraphNodeDe {
    id: NodeId,
    node_type: String,
    #[serde(default)]
    type_var_map: HashMap<TypeVarId, TypeVarDefinition>,
    #[serde(default)]
    position: NodePosition,
    #[serde(default)]
    instance_params: NodeInstanceParams,
    #[serde(default)]
    pins: Vec<PinInstance>,
}

impl Serialize for GraphInstance {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let data_state = self
            .data_state
            .read()
            .map_err(|_| serde::ser::Error::custom("data_state lock poisoned"))?;

        // 节点按 id 排序，保证落盘 JSON 稳定（避免 HashMap 迭代顺序导致的伪 diff）
        let mut nodes: Vec<&NodeInstance> = data_state.nodes.values().collect();
        nodes.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string()));

        let node_ser: Vec<GraphNodeSer> = nodes
            .iter()
            .map(|node| {
                let pins: Vec<&PinInstance> = node
                    .pin_ids
                    .iter()
                    .filter_map(|pin_id| data_state.pins.get(pin_id))
                    .collect();
                GraphNodeSer {
                    id: node.id,
                    node_type: &node.definition.node_type,
                    type_var_map: &node.type_var_map,
                    position: node.position.clone(),
                    instance_params: &node.instance_params,
                    pins,
                }
            })
            .collect();

        let mut connections = data_state.connections.all_connections();
        connections.sort_by(|a, b| {
            (a.from_pin.to_string(), a.to_pin.to_string())
                .cmp(&(b.from_pin.to_string(), b.to_pin.to_string()))
        });

        GraphDocSer {
            name: &self.name,
            kind: self.kind.clone(),
            function_inputs: &self.function_inputs,
            function_outputs: &self.function_outputs,
            nodes: node_ser,
            connections,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GraphInstance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let doc = GraphDocDe::deserialize(deserializer)?;
        Ok(Self::from_persisted_parts(
            GraphResourcePath::from_normalized_unchecked(String::new()),
            doc.name,
            doc.kind,
            doc.function_inputs,
            doc.function_outputs,
            doc.nodes,
            doc.connections,
        ))
    }
}

impl GraphInstance {
    /// 从持久化的扁平节点 + 连接重建 `GraphInstance`（无 registry，
    /// 静态 pin 的完整定义随后由 `set_registry` 重挂）。
    pub(super) fn from_persisted_parts(
        resource_path: GraphResourcePath,
        name: String,
        kind: GraphKind,
        function_inputs: Vec<FunctionSignaturePin>,
        function_outputs: Vec<FunctionSignaturePin>,
        nodes: Vec<GraphNodeDe>,
        connections: Vec<Connection>,
    ) -> Self {
        let mut data_state = GraphDataState::default();

        for node in nodes {
            let node_id = node.id;
            let definition = Arc::new(NodeDefinition::placeholder(node.node_type));
            let pin_ids: Vec<PinId> = node.pins.iter().map(|pin| pin.id).collect();

            for pin in node.pins {
                data_state.connections.register_pin(pin.id, node_id);
                data_state.pins.insert(pin.id, pin);
            }

            data_state.add_node(NodeInstance {
                id: node_id,
                definition,
                type_var_map: node.type_var_map,
                position: node.position,
                instance_params: node.instance_params,
                pin_ids,
            });
        }

        for connection in connections {
            data_state
                .connections
                .connect(connection.from_pin, connection.to_pin);
        }

        Self {
            resource_path,
            name,
            kind,
            function_inputs,
            function_outputs,
            data_state: Arc::new(RwLock::new(data_state)),
            registry: Default::default(),
            schema_provider: None,
            runtime_prepared_epoch: 0,
        }
    }
}
