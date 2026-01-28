//! 执行上下文模块
//!
//! 负责图的执行逻辑。

use crate::executor::{
    get_all_node_definitions, ExecutionContextTrait, GraphData, NodeData, NodeDefinition,
};
use serde_json::Value;
use std::collections::HashMap;
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_log::log::info;

/// 执行上下文
pub struct ExecutionContext {
    /// 变量存储
    pub variables: HashMap<String, Value>,
    /// 执行日志
    pub logs: Vec<String>,
    /// 节点映射 (node_id -> NodeData)
    nodes: HashMap<String, NodeData>,
    /// 针脚到节点的映射 (pin_id -> node_id)
    pin_to_node: HashMap<String, String>,
    /// 节点定义缓存 (node_type -> NodeDefinition)
    definitions: HashMap<String, NodeDefinition>,
    /// 调用栈
    call_stack: Vec<String>,
    /// Tauri 应用句柄（可选）
    app_handle: Option<AppHandle>,
}

impl ExecutionContext {
    /// 创建新的执行上下文
    pub fn new(graph: GraphData) -> Self {
        let mut nodes = HashMap::new();
        let mut pin_to_node = HashMap::new();
        let mut initial_vars = HashMap::new();

        // 初始化变量
        if let Some(vars) = graph.variables {
            for (id, var_data) in vars {
                initial_vars.insert(id, var_data.value);
            }
        }

        // 建立节点和针脚映射
        for node in graph.nodes {
            for pin in &node.inputs {
                pin_to_node.insert(pin.id.clone(), node.id.clone());
            }
            for pin in &node.outputs {
                pin_to_node.insert(pin.id.clone(), node.id.clone());
            }
            nodes.insert(node.id.clone(), node);
        }

        // 加载所有节点定义
        let mut def_map = HashMap::new();
        for def in get_all_node_definitions() {
            def_map.insert(def.node_type.clone(), def);
        }

        Self {
            variables: initial_vars,
            logs: Vec::new(),
            nodes,
            pin_to_node,
            definitions: def_map,
            call_stack: Vec::new(),
            app_handle: None,
        }
    }

    /// 设置 Tauri 应用句柄（用于支持窗口操作）
    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
    }

    /// 执行图
    pub fn execute(&mut self) -> Result<Vec<String>, String> {
        let start_node_id = self
            .nodes
            .values()
            .find(|n| n.node_type == "event_on_run")
            .map(|n| n.id.clone())
            .ok_or("No 'event_on_run' node found")?;

        let log_msg = format!("Starting execution from node: {}", start_node_id);
        info!("{}", log_msg);
        self.logs.push(log_msg);
        self.run_flow_internal(&start_node_id, "Exec")?;

        info!("Execution finished");
        self.logs.push("Execution finished".to_string());
        Ok(self.logs.clone())
    }

    /// 内部：执行流程
    fn run_flow_internal(&mut self, node_id: &str, output_exec_name: &str) -> Result<(), String> {
        let node = self.nodes.get(node_id).ok_or("Node not found")?.clone();
        let log_msg = format!(">>> Executing Node: {} ({})", node.title, node.node_type);
        info!("{}", log_msg);
        self.logs.push(log_msg);

        let def = self
            .definitions
            .get(&node.node_type)
            .ok_or(format!("Definition not found: {}", node.node_type))?
            .clone();

        let next_exec_name = if let Some(processor) = def.flow_processor {
            processor(self, &node)?
        } else {
            output_exec_name.to_string()
        };

        // 如果是返回操作
        if next_exec_name == "__RETURN__" {
            return Ok(());
        }
        if !next_exec_name.is_empty() {
            self.trigger_next_flow(node_id, &next_exec_name)?;
        }
        Ok(())
    }

    /// 触发下一个流程节点
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
                return self.run_flow_internal(&next_node_id, "Out");
            }
        }
        Ok(())
    }

    /// 计算节点输出值
    fn evaluate_node_output(&mut self, node_id: &str, pin_id: &str) -> Value {
        let node = self.nodes.get(node_id).unwrap().clone();

        let def = match self.definitions.get(&node.node_type) {
            Some(d) => d.clone(),
            None => return Value::Null,
        };

        let val = if let Some(processor) = def.data_processor {
            processor(self, &node, pin_id)
        } else {
            Value::Null
        };

        let pin_name = node.inputs.iter().chain(node.outputs.iter())
            .find(|p| p.id == pin_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| pin_id.to_string());

        let log_msg = format!(
            "  [Data] Node '{}' pin '{}' -> {:?}",
            node.title, pin_name, val
        );
        info!("{}", log_msg);
        self.logs.push(log_msg);
        val
    }
}

// 实现 ExecutionContextTrait
impl ExecutionContextTrait for ExecutionContext {
    fn get_pin_value(&mut self, pin_id: &str) -> Value {
        let node_id = match self.pin_to_node.get(pin_id) {
            Some(id) => id.clone(),
            None => return Value::Null,
        };

        let node = self.nodes.get(&node_id).unwrap().clone();

        // 1. 检查是否是输出针脚。如果是输出针脚，直接计算并返回。
        if let Some(output_pin) = node.outputs.iter().find(|p| p.id == pin_id) {
            // 特殊处理 A：如果是 function_entry 或 macro_inputs 的输出针脚，需要从调用端获取数据
            if (node.node_type == "function_entry" || node.node_type == "macro_inputs")
                && !self.call_stack.is_empty()
            {
                let caller_node_id = self.call_stack.last().unwrap().clone();
                let caller_node = self.nodes.get(&caller_node_id).unwrap().clone();
                if let Some(caller_input) = caller_node.inputs.iter().find(|tip| tip.name == output_pin.name)
                {
                    return self.get_pin_value(&caller_input.id);
                }
            }
            
            // 特殊处理 B：如果是 call_function 或 call_macro 的输出针脚，需要从函数内部的 return/outputs 节点获取
            let sub_graph_id = node.sub_graph_id.clone();
            let return_node_type = if node.node_type == "call_function" { "function_return" } else { "macro_outputs" };
            
            // 找到对应的 return 节点中的输入针脚 ID
            let target_pin_id = self.nodes.values()
                .find(|n| n.node_type == return_node_type && n.sub_graph_id == sub_graph_id)
                .and_then(|ret_node| {
                    ret_node.inputs.iter()
                        .find(|tip| tip.name == output_pin.name)
                        .map(|tip| tip.id.clone())
                });
            
            if let Some(pin_id) = target_pin_id {
                return self.get_pin_value(&pin_id);
            }

            // 普通输出针脚，调用节点的数据处理器来计算结果
            return self.evaluate_node_output(&node_id, pin_id);
        }

        // 2. 如果是输入针脚，则需要沿着连接找到上游的输出针脚
        if let Some(input_pin) = node.inputs.iter().find(|p| p.id == pin_id) {
            if input_pin.links.is_empty() {
                return input_pin.default_value.clone().unwrap_or(Value::Null);
            }

            // 获取第一个连接的上游针脚 ID
            let source_pin_id = &input_pin.links[0];
            return self.get_pin_value(source_pin_id);
        }

        Value::Null
    }

    fn get_variable(&self, var_id: &str) -> Option<&Value> {
        self.variables.get(var_id)
    }

    fn set_variable(&mut self, var_id: &str, value: Value) -> bool {
        if self.variables.contains_key(var_id) {
            self.variables.insert(var_id.to_string(), value);
            true
        } else {
            false
        }
    }

    fn log(&mut self, message: String) {
        info!("{}", message);
        self.logs.push(message);
    }

    fn run_flow(&mut self, node_id: &str, output_pin: &str) -> Result<(), String> {
        self.trigger_next_flow(node_id, output_pin)
    }

    fn push_call_stack(&mut self, node_id: String) {
        self.call_stack.push(node_id);
    }

    fn pop_call_stack(&mut self) {
        self.call_stack.pop();
    }

    fn get_call_stack_top(&self) -> Option<&String> {
        self.call_stack.last()
    }

    fn find_node_by(&self, predicate: &dyn Fn(&NodeData) -> bool) -> Option<String> {
        self.nodes
            .values()
            .find(|n| predicate(n))
            .map(|n| n.id.clone())
    }

    fn open_window(&mut self, label: String, title: String, url: String) -> Result<(), String> {
        let app_handle = self
            .app_handle
            .as_ref()
            .ok_or("AppHandle not available in execution context")?
            .clone();

        let log_msg = format!("Opening window: {} ({})", title, url);
        info!("{}", log_msg);
        self.logs.push(log_msg.clone());

        // 在新线程中创建窗口，避免阻塞执行流程
        let label_clone = label.clone();
        let title_clone = title.clone();
        let url_clone = url.clone();
        
        std::thread::spawn(move || {
            // 创建新窗口
            match WebviewWindowBuilder::new(
                &app_handle,
                label_clone.clone(),
                WebviewUrl::App(url_clone.into()),
            )
            .title(title_clone)
            .inner_size(800.0, 600.0)
            .min_inner_size(400.0, 300.0)
            .resizable(true)
            .visible(false)
            .decorations(false)  // 禁用系统窗口装饰，使用自定义标题栏
            .transparent(false)  // 不透明背景
            .center()            // 居中显示
            .build() {
                Ok(_) => {
                    info!("Window '{}' opened successfully", label_clone);
                }
                Err(e) => {
                    info!("Failed to create window '{}': {}", label_clone, e);
                }
            }
        });

        // 立即返回，不等待窗口创建完成
        let success_msg = format!("Window '{}' creation initiated", label);
        info!("{}", success_msg);
        self.logs.push(success_msg);

        Ok(())
    }
}
