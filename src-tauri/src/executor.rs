use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// --- 类型定义 ---

/// 数据节点处理器：输入 (上下文, 节点数据, 请求的针脚ID) -> 返回值
pub type DataProcessor = fn(&mut ExecutionContext, &NodeData, &str) -> Value;

/// 逻辑流处理器：输入 (上下文, 节点数据) -> 返回下一步要执行的输出针脚名称
pub type FlowProcessor = fn(&mut ExecutionContext, &NodeData) -> Result<String, String>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinData {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    pub links: Vec<String>,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<Value>,
}

/// 节点元数据定义
#[derive(Clone)] // 移除 Serialize/Deserialize，手动实现以处理函数指针
pub struct NodeDefinition {
    pub node_type: String,
    pub category: String,
    pub title: String,
    pub inputs: Vec<PinDefinition>,
    pub outputs: Vec<PinDefinition>,
    pub ui_style: String,
    pub description: Option<String>,
    // 执行逻辑（不参与序列化）
    pub data_processor: Option<DataProcessor>,
    pub flow_processor: Option<FlowProcessor>,
}

// 为 NodeDefinition 手动实现 Serialize，以便通过 Tauri 发送给前端
impl Serialize for NodeDefinition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct NodeDefProxy<'a> {
            node_type: &'a String,
            category: &'a String,
            title: &'a String,
            inputs: &'a Vec<PinDefinition>,
            outputs: &'a Vec<PinDefinition>,
            ui_style: &'a String,
            description: &'a Option<String>,
        }
        let proxy = NodeDefProxy {
            node_type: &self.node_type,
            category: &self.category,
            title: &self.title,
            inputs: &self.inputs,
            outputs: &self.outputs,
            ui_style: &self.ui_style,
            description: &self.description,
        };
        proxy.serialize(serializer)
    }
}

// 反序列化对于 Definition 来说通常不需要（因为是从后端发往前端）
impl<'de> Deserialize<'de> for NodeDefinition {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Err(serde::de::Error::custom(
            "NodeDefinition cannot be deserialized",
        ))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeData {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    pub inputs: Vec<PinData>,
    pub outputs: Vec<PinData>,
    #[serde(rename = "variableId")]
    pub variable_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VariableData {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphData {
    pub version: String,
    pub nodes: Vec<NodeData>,
    pub variables: Option<HashMap<String, VariableData>>,
}

pub struct ExecutionContext {
    pub variables: HashMap<String, Value>,
    pub logs: Vec<String>,
    nodes: HashMap<String, NodeData>,
    pin_to_node: HashMap<String, String>,
    definitions: HashMap<String, NodeDefinition>, // 运行时缓存定义
}

impl ExecutionContext {
    pub fn new(graph: GraphData) -> Self {
        let mut nodes = HashMap::new();
        let mut pin_to_node = HashMap::new();
        let mut initial_vars = HashMap::new();

        if let Some(vars) = graph.variables {
            for (id, var_data) in vars {
                initial_vars.insert(id, var_data.value);
            }
        }

        for node in graph.nodes {
            for pin in &node.inputs {
                pin_to_node.insert(pin.id.clone(), node.id.clone());
            }
            for pin in &node.outputs {
                pin_to_node.insert(pin.id.clone(), node.id.clone());
            }
            nodes.insert(node.id.clone(), node);
        }

        // 加载所有定义
        let mut def_map = HashMap::new();
        for def in Self::get_definitions() {
            def_map.insert(def.node_type.clone(), def);
        }

        Self {
            variables: initial_vars,
            logs: Vec::new(),
            nodes,
            pin_to_node,
            definitions: def_map,
        }
    }

    pub fn get_pin_value(&mut self, pin_id: &str) -> Value {
        let node_id = match self.pin_to_node.get(pin_id) {
            Some(id) => id.clone(),
            None => return Value::Null,
        };

        let node = self.nodes.get(&node_id).unwrap().clone();
        let pin = node
            .inputs
            .iter()
            .chain(node.outputs.iter())
            .find(|p| p.id == pin_id)
            .unwrap();

        if pin.links.is_empty() {
            return pin.default_value.clone().unwrap_or(Value::Null);
        }

        let source_pin_id = &pin.links[0];
        let source_node_id = match self.pin_to_node.get(source_pin_id) {
            Some(id) => id.clone(),
            None => return Value::Null,
        };

        self.evaluate_node_output(&source_node_id, source_pin_id)
    }

    /// 核心改进：直接调用定义中的 data_processor
    fn evaluate_node_output(&mut self, node_id: &str, pin_id: &str) -> Value {
        let node = self.nodes.get(node_id).unwrap().clone();
        let def = match self.definitions.get(&node.node_type) {
            Some(d) => d,
            None => return Value::Null,
        };

        let val = if let Some(processor) = def.data_processor {
            processor(self, &node, pin_id)
        } else {
            Value::Null
        };

        self.logs.push(format!(
            "  [Data] Node {} output pin {} -> {:?}",
            node.title, pin_id, val
        ));
        val
    }

    pub fn execute(&mut self) -> Result<Vec<String>, String> {
        let start_node_id = self
            .nodes
            .values()
            .find(|n| n.node_type == "on_start")
            .map(|n| n.id.clone())
            .ok_or("No 'on_start' node found")?;

        self.logs
            .push(format!("Starting execution from node: {}", start_node_id));
        self.run_flow(&start_node_id, "Out")?;

        self.logs.push("Execution finished".to_string());
        Ok(self.logs.clone())
    }

    /// 核心改进：直接调用定义中的 flow_processor
    fn run_flow(&mut self, node_id: &str, output_exec_name: &str) -> Result<(), String> {
        let node = self.nodes.get(node_id).ok_or("Node not found")?.clone();
        self.logs.push(format!(
            ">>> Executing Node: {} ({})",
            node.title, node.node_type
        ));

        let def = self
            .definitions
            .get(&node.node_type)
            .ok_or("Definition not found")?;

        let next_exec_name = if let Some(processor) = def.flow_processor {
            processor(self, &node)?
        } else {
            output_exec_name.to_string()
        };

        self.trigger_next_flow(node_id, &next_exec_name)
    }

    fn trigger_next_flow(&mut self, node_id: &str, pin_name: &str) -> Result<(), String> {
        let node = self.nodes.get(node_id).unwrap();
        let exec_pin = node
            .outputs
            .iter()
            .find(|p| p.pin_type == "exec" && p.name.to_lowercase() == pin_name.to_lowercase());

        if let Some(pin) = exec_pin {
            if !pin.links.is_empty() {
                let next_pin_id = &pin.links[0];
                let next_node_id = self
                    .pin_to_node
                    .get(next_pin_id)
                    .cloned()
                    .ok_or("Target node not found")?;
                return self.run_flow(&next_node_id, "Out");
            }
        }
        Ok(())
    }

    // --- 节点定义库 (注册中心) ---

    pub fn get_definitions() -> Vec<NodeDefinition> {
        vec![
            NodeDefinition {
                node_type: "on_start".into(),
                category: "Event".into(),
                title: "On Start".into(),
                ui_style: "event".into(),
                description: Some("Execution entry point".into()),
                inputs: vec![],
                outputs: vec![PinDefinition {
                    name: "Out".into(),
                    pin_type: "exec".into(),
                    default_value: None,
                }],
                data_processor: None,
                flow_processor: Some(|_ctx, _node| Ok("Out".to_string())),
            },
            NodeDefinition {
                node_type: "print".into(),
                category: "Function".into(),
                title: "Print".into(),
                ui_style: "default".into(),
                description: Some("Print a value to the log".into()),
                inputs: vec![
                    PinDefinition {
                        name: "In".into(),
                        pin_type: "exec".into(),
                        default_value: None,
                    },
                    PinDefinition {
                        name: "Value".into(),
                        pin_type: "string".into(),
                        default_value: Some(Value::String("".into())),
                    },
                ],
                outputs: vec![PinDefinition {
                    name: "Out".into(),
                    pin_type: "exec".into(),
                    default_value: None,
                }],
                data_processor: None,
                flow_processor: Some(|ctx, node| {
                    let data_pin = node
                        .inputs
                        .iter()
                        .find(|p| p.pin_type != "exec")
                        .ok_or("Print node missing data input")?;
                    let val = ctx.get_pin_value(&data_pin.id);
                    let output = if let Value::String(s) = &val {
                        s.clone()
                    } else {
                        val.to_string()
                    };
                    ctx.logs.push(format!("[NODE PRINT]: {}", output));
                    println!("[NODE PRINT]: {}", output);
                    Ok("Out".to_string())
                }),
            },
            NodeDefinition {
                node_type: "add".into(),
                category: "Math".into(),
                title: "Add".into(),
                ui_style: "math".into(),
                description: Some("Add two numbers".into()),
                inputs: vec![
                    PinDefinition {
                        name: "A".into(),
                        pin_type: "float".into(),
                        default_value: Some(Value::from(0.0)),
                    },
                    PinDefinition {
                        name: "B".into(),
                        pin_type: "float".into(),
                        default_value: Some(Value::from(0.0)),
                    },
                ],
                outputs: vec![PinDefinition {
                    name: "Sum".into(),
                    pin_type: "float".into(),
                    default_value: None,
                }],
                data_processor: Some(|ctx, node, _pin_id| {
                    let a = ctx
                        .get_pin_value(&node.inputs[0].id)
                        .as_f64()
                        .unwrap_or(0.0);
                    let b = ctx
                        .get_pin_value(&node.inputs[1].id)
                        .as_f64()
                        .unwrap_or(0.0);
                    Value::from(a + b)
                }),
                flow_processor: None,
            },
            NodeDefinition {
                node_type: "if_else".into(),
                category: "Branch".into(),
                title: "Branch".into(),
                ui_style: "default".into(),
                description: Some("Branch execution based on condition".into()),
                inputs: vec![
                    PinDefinition {
                        name: "In".into(),
                        pin_type: "exec".into(),
                        default_value: None,
                    },
                    PinDefinition {
                        name: "Cond".into(),
                        pin_type: "bool".into(),
                        default_value: Some(Value::Bool(false)),
                    },
                ],
                outputs: vec![
                    PinDefinition {
                        name: "True".into(),
                        pin_type: "exec".into(),
                        default_value: None,
                    },
                    PinDefinition {
                        name: "False".into(),
                        pin_type: "exec".into(),
                        default_value: None,
                    },
                ],
                data_processor: None,
                flow_processor: Some(|ctx, node| {
                    let data_pin = node
                        .inputs
                        .iter()
                        .find(|p| p.pin_type != "exec")
                        .ok_or("Branch node missing data input")?;
                    let val = ctx.get_pin_value(&data_pin.id);
                    let condition = val
                        .as_bool()
                        .unwrap_or_else(|| val.as_f64().unwrap_or(0.0) != 0.0);
                    let next = if condition { "True" } else { "False" };
                    ctx.logs.push(format!(
                        "  Branch condition is {}, moving to '{}'",
                        condition, next
                    ));
                    Ok(next.to_string())
                }),
            },
            NodeDefinition {
                node_type: "get_variable".into(),
                category: "Variable".into(),
                title: "Get Variable".into(),
                ui_style: "default".into(),
                description: Some("Get variable value".into()),
                inputs: vec![],
                outputs: vec![PinDefinition {
                    name: "Value".into(),
                    pin_type: "object".into(),
                    default_value: None,
                }],
                data_processor: Some(|ctx, node, _pin_id| {
                    let var_id = node
                        .variable_id
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| "default_var".to_string());
                    match ctx.variables.get(&var_id) {
                        Some(val) => val.clone(),
                        None => {
                            ctx.logs.push(format!("[Error] Variable '{}' not found", var_id));
                            Value::Null
                        }
                    }
                }),
                flow_processor: None,
            },
            NodeDefinition {
                node_type: "set_variable".into(),
                category: "Variable".into(),
                title: "Set Variable".into(),
                ui_style: "default".into(),
                description: Some("Set variable value".into()),
                inputs: vec![
                    PinDefinition {
                        name: "In".into(),
                        pin_type: "exec".into(),
                        default_value: None,
                    },
                    PinDefinition {
                        name: "Value".into(),
                        pin_type: "object".into(),
                        default_value: None,
                    },
                ],
                outputs: vec![
                    PinDefinition {
                        name: "Out".into(),
                        pin_type: "exec".into(),
                        default_value: None,
                    },
                    PinDefinition {
                        name: "Value".into(),
                        pin_type: "object".into(),
                        default_value: None,
                    },
                ],
                data_processor: Some(|ctx, node, _pin_id| {
                    // Set 节点的输出 Value 通常返回其设置后的值
                    let var_id = node
                        .variable_id
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| "default_var".to_string());
                    ctx.variables.get(&var_id).cloned().unwrap_or(Value::Null)
                }),
                flow_processor: Some(|ctx, node| {
                    let data_pin = node
                        .inputs
                        .iter()
                        .find(|p| p.pin_type != "exec")
                        .ok_or("Set Variable missing input")?;
                    let val = ctx.get_pin_value(&data_pin.id);
                    let var_id = node
                        .variable_id
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| "default_var".to_string());
                    
                    if ctx.variables.contains_key(&var_id) {
                        ctx.variables.insert(var_id, val);
                        Ok("Out".to_string())
                    } else {
                        let err = format!("[Error] Cannot set unknown variable '{}'", var_id);
                        ctx.logs.push(err.clone());
                        Err(err)
                    }
                }),
            },
        ]
    }
}
