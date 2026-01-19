use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
}

impl ExecutionContext {
    pub fn new(graph: GraphData) -> Self {
        let mut nodes = HashMap::new();
        let mut pin_to_node = HashMap::new();
        let mut initial_vars = HashMap::new();

        // 加载初始变量值
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

        Self {
            variables: initial_vars,
            logs: Vec::new(),
            nodes,
            pin_to_node,
        }
    }

    /// 获取某个 Pin 的值（如果连接了则溯源计算，否则取默认值）
    pub fn get_pin_value(&mut self, pin_id: &str) -> Value {
        let node_id = match self.pin_to_node.get(pin_id) {
            Some(id) => id.clone(),
            None => return Value::Null,
        };

        let node = self.nodes.get(&node_id).unwrap().clone();
        let pin = node.inputs.iter().chain(node.outputs.iter())
            .find(|p| p.id == pin_id)
            .unwrap();

        if pin.links.is_empty() {
            return pin.default_value.clone().unwrap_or(Value::Null);
        }

        // 溯源：找到连接的输出 Pin
        let source_pin_id = &pin.links[0];
        let source_node_id = match self.pin_to_node.get(source_pin_id) {
            Some(id) => id.clone(),
            None => return Value::Null,
        };

        self.evaluate_node_output(&source_node_id, source_pin_id)
    }

    /// 计算节点的某个输出值
    fn evaluate_node_output(&mut self, node_id: &str, pin_id: &str) -> Value {
        let node = self.nodes.get(node_id).unwrap().clone();
        
        let val = match node.node_type.as_str() {
            "add" => {
                let a = self.get_pin_value(&node.inputs[0].id);
                let b = self.get_pin_value(&node.inputs[1].id);
                let sum = a.as_f64().unwrap_or(0.0) + b.as_f64().unwrap_or(0.0);
                Value::from(sum)
            }
            "get_variable" => {
                let var_id = node.variable_id.unwrap_or_else(|| "default_var".to_string());
                let val = self.variables.get(&var_id).cloned().unwrap_or_else(|| {
                    let err = format!("Variable with ID {} not found", var_id);
                    self.logs.push(format!("  [Error] {}", err));
                    Value::from(0)
                });
                self.logs.push(format!("  [Data] Read Variable {} -> {:?}", var_id, val));
                val
            }
            "int_to_bool" => {
                let val = self.get_pin_value(&node.inputs[0].id);
                // 兼容处理：支持 i64 和 f64
                let is_nonzero = if let Some(i) = val.as_i64() {
                    i != 0
                } else if let Some(f) = val.as_f64() {
                    f != 0.0
                } else if let Some(b) = val.as_bool() {
                    b
                } else {
                    false
                };
                Value::from(is_nonzero)
            }
            _ => Value::Null,
        };

        self.logs.push(format!("  [Data] Node {} output pin {} -> {:?}", node.title, pin_id, val));
        val
    }

    /// 执行逻辑流
    pub fn execute(&mut self) -> Result<Vec<String>, String> {
        // 1. 寻找入口点 (On Start 节点)
        let start_node_id = self.nodes.values()
            .find(|n| n.node_type == "on_start")
            .map(|n| n.id.clone())
            .ok_or("No 'on_start' node found")?;

        self.logs.push(format!("Starting execution from node: {}", start_node_id));
        
        // 寻找该节点的第一个 exec 输出 pin
        let start_node = self.nodes.get(&start_node_id).unwrap();
        let exec_pin_name = start_node.outputs.iter()
            .find(|p| p.pin_type == "exec")
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Out".to_string());

        self.run_flow(&start_node_id, &exec_pin_name)?;

        self.logs.push("Execution finished".to_string());
        Ok(self.logs.clone())
    }

    fn run_flow(&mut self, node_id: &str, output_exec_name: &str) -> Result<(), String> {
        let node = self.nodes.get(node_id).ok_or("Node not found")?.clone();
        self.logs.push(format!(">>> Executing Node: {} ({})", node.title, node.node_type));
        
        // 执行当前节点的副作用逻辑
        match node.node_type.as_str() {
            "set_variable" => {
                let data_pin = node.inputs.iter().find(|p| p.pin_type != "exec")
                    .ok_or("Set Variable node missing data input")?;
                let val = self.get_pin_value(&data_pin.id);
                let var_id = node.variable_id.unwrap_or_else(|| "default_var".to_string());
                self.logs.push(format!("  Setting variable {} to {:?}", var_id, val));
                self.variables.insert(var_id, val);
            }
            "print" => {
                let data_pin = node.inputs.iter().find(|p| p.pin_type != "exec")
                    .ok_or("Print node missing data input")?;
                let val = self.get_pin_value(&data_pin.id);
                let output = match &val {
                    Value::String(s) => s.clone(),
                    _ => val.to_string(),
                };
                self.logs.push(format!("[NODE PRINT]: {}", output));
                println!("[NODE PRINT]: {}", output);
            }
            "if_else" => {
                let data_pin = node.inputs.iter().find(|p| p.pin_type != "exec")
                    .ok_or("Branch node missing data input")?;
                let val = self.get_pin_value(&data_pin.id);
                let condition = if let Some(b) = val.as_bool() {
                    b
                } else if let Some(i) = val.as_i64() {
                    i != 0
                } else if let Some(f) = val.as_f64() {
                    f != 0.0
                } else {
                    false
                };
                
                let next_exec_pin = if condition { "true" } else { "false" };
                self.logs.push(format!("  Branch condition is {}, moving to '{}'", condition, next_exec_pin));
                return self.trigger_next_flow(node_id, next_exec_pin);
            }
            _ => {}
        }

        // 默认尝试从指定的 exec 输出向下游执行
        self.trigger_next_flow(node_id, output_exec_name)
    }

    fn trigger_next_flow(&mut self, node_id: &str, pin_name: &str) -> Result<(), String> {
        let node = self.nodes.get(node_id).unwrap();
        let exec_pin = node.outputs.iter().find(|p| p.pin_type == "exec" && p.name.to_lowercase() == pin_name.to_lowercase());
        
        if let Some(pin) = exec_pin {
            if !pin.links.is_empty() {
                let next_pin_id = &pin.links[0];
                let next_node_id = self.pin_to_node.get(next_pin_id).cloned().ok_or("Target node not found")?;
                return self.run_flow(&next_node_id, "Out");
            }
        }
        Ok(())
    }
}
