//! 执行上下文模块
//!
//! 负责图的执行逻辑，使用运行时节点对象（GenericNode）而不是序列化数据。

use crate::executor::{
    BasePin, ConnectionManager, ExecutionContextTrait, GenericNode, GraphData, Node, NodeData,
    NodeId, PinId,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_log::log::info;

/// 执行上下文
pub struct ExecutionContext {
    /// 运行时节点（node_id -> GenericNode）
    nodes: HashMap<NodeId, Arc<Mutex<GenericNode>>>,

    /// 连接管理器
    connection_manager: Arc<ConnectionManager>,

    /// 原始数据 ID 到运行时 ID 的映射
    data_id_to_runtime_id: HashMap<String, NodeId>,

    /// 运行时 ID 到原始数据 ID 的映射
    runtime_id_to_data_id: HashMap<NodeId, String>,

    /// Pin ID 到运行时节点 ID 的映射
    pin_to_node: HashMap<PinId, NodeId>,

    /// 前端 Pin ID (字符串) 到运行时 PinId 的映射
    data_pin_id_to_runtime_pin_id: HashMap<String, PinId>,

    /// 变量存储
    pub variables: HashMap<String, Value>,

    /// 执行日志
    pub logs: Vec<String>,

    /// 调用栈
    call_stack: Vec<String>,

    /// 当前执行栈（用于检测循环）
    execution_stack: Vec<NodeId>,

    /// Tauri 应用句柄（可选）
    app_handle: Option<AppHandle>,
}

impl ExecutionContext {
    /// 创建新的执行上下文
    pub fn new(graph: GraphData) -> Self {
        let mut ctx = Self {
            nodes: HashMap::new(),
            connection_manager: Arc::new(ConnectionManager::new()),
            data_id_to_runtime_id: HashMap::new(),
            runtime_id_to_data_id: HashMap::new(),
            pin_to_node: HashMap::new(),
            data_pin_id_to_runtime_pin_id: HashMap::new(),
            variables: HashMap::new(),
            logs: Vec::new(),
            call_stack: Vec::new(),
            execution_stack: Vec::new(),
            app_handle: None,
        };

        // 初始化变量
        if let Some(vars) = graph.variables {
            for (id, var_data) in vars {
                ctx.variables.insert(id, var_data.value);
            }
        }

        // 创建所有节点
        for node_data in &graph.nodes {
            if let Err(e) = ctx.create_node_from_data(node_data) {
                ctx.log(format!("[ERROR] Failed to create node: {}", e));
            }
        }

        // 建立连接
        for node_data in &graph.nodes {
            if let Err(e) = ctx.create_connections_from_data(node_data) {
                ctx.log(format!("[ERROR] Failed to create connections: {}", e));
            }
        }

        ctx
    }

    /// 从 NodeData 创建运行时节点
    fn create_node_from_data(&mut self, node_data: &NodeData) -> Result<NodeId, String> {
        use uuid::Uuid;

        let runtime_id = Uuid::new_v4();
        let node = GenericNode::new(runtime_id, &node_data.title, &node_data.node_type);
        node.set_variable_id(node_data.variable_id.clone());

        // 创建输入 Pin
        for pin_data in &node_data.inputs {
            if pin_data.pin_type == "exec" {
                use crate::executor::pin::GenericInExecPin;
                let exec_pin = GenericInExecPin::new(runtime_id, &pin_data.name);
                let pin_id = exec_pin.id();
                node.add_in_exec_pin(exec_pin);
                self.pin_to_node.insert(pin_id, runtime_id);
                self.data_pin_id_to_runtime_pin_id
                    .insert(pin_data.id.clone(), pin_id);
            } else {
                use crate::executor::pin::GenericInDataPin;
                let pin = GenericInDataPin::new(runtime_id, &pin_data.name, &pin_data.pin_type);
                let pin_id = pin.id();
                node.add_input(pin);
                self.pin_to_node.insert(pin_id, runtime_id);
                self.data_pin_id_to_runtime_pin_id
                    .insert(pin_data.id.clone(), pin_id);
            }
        }

        // 创建输出 Pin
        for pin_data in &node_data.outputs {
            if pin_data.pin_type == "exec" {
                use crate::executor::pin::GenericOutExecPin;
                let exec_pin = GenericOutExecPin::new(runtime_id, &pin_data.name);
                let pin_id = exec_pin.id();
                node.add_out_exec_pin(exec_pin);
                self.pin_to_node.insert(pin_id, runtime_id);
                self.data_pin_id_to_runtime_pin_id
                    .insert(pin_data.id.clone(), pin_id);
            } else {
                use crate::executor::pin::GenericOutDataPin;
                let pin = GenericOutDataPin::new(runtime_id, &pin_data.name, &pin_data.pin_type);
                let pin_id = pin.id();
                node.add_output(pin);
                self.pin_to_node.insert(pin_id, runtime_id);
                self.data_pin_id_to_runtime_pin_id
                    .insert(pin_data.id.clone(), pin_id);
            }
        }

        // 注册节点到连接管理器
        self.connection_manager
            .register_node(&node)
            .map_err(|e| format!("Failed to register node: {:?}", e))?;

        // 存储节点
        self.nodes.insert(runtime_id, Arc::new(Mutex::new(node)));

        // 映射 ID
        self.data_id_to_runtime_id
            .insert(node_data.id.clone(), runtime_id);
        self.runtime_id_to_data_id
            .insert(runtime_id, node_data.id.clone());

        Ok(runtime_id)
    }

    /// 从 NodeData 创建连接
    fn create_connections_from_data(&mut self, node_data: &NodeData) -> Result<(), String> {
        // 遍历所有输出 Pin，建立连接
        for pin_data in &node_data.outputs {
            // 获取源 Pin 的运行时 ID
            let from_runtime_pin_id = match self.data_pin_id_to_runtime_pin_id.get(&pin_data.id) {
                Some(&id) => id,
                None => {
                    self.log(format!(
                        "[WARN] Source pin '{}' not found in runtime",
                        pin_data.id
                    ));
                    continue;
                }
            };

            // 遍历 links，建立连接
            for target_pin_id in &pin_data.links {
                // 获取目标 Pin 的运行时 ID
                let to_runtime_pin_id = match self.data_pin_id_to_runtime_pin_id.get(target_pin_id)
                {
                    Some(&id) => id,
                    None => {
                        self.log(format!(
                            "[WARN] Target pin '{}' not found in runtime",
                            target_pin_id
                        ));
                        continue;
                    }
                };

                // 建立连接
                if let Err(e) = self
                    .connection_manager
                    .connect_by_id(from_runtime_pin_id, to_runtime_pin_id)
                {
                    self.log(format!("[ERROR] Failed to connect pins: {:?}", e));
                } else {
                    self.log(format!("[Connection] {} -> {}", pin_data.id, target_pin_id));
                }
            }
        }
        Ok(())
    }

    /// 设置 Tauri 应用句柄（用于支持窗口操作）
    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
    }

    /// 执行图
    pub fn execute(&mut self) -> Result<Vec<String>, String> {
        self.log_graph_structure();

        // 查找起始节点
        let start_runtime_id = self
            .nodes
            .iter()
            .find(|(_, node)| {
                let node_guard = node.lock().unwrap();
                node_guard.node_type() == "event_on_run"
            })
            .map(|(id, _)| *id)
            .ok_or("No 'event_on_run' node found")?;

        let log_msg = format!("Starting execution from node: {:?}", start_runtime_id);
        info!("{}", log_msg);
        self.logs.push(log_msg);

        self.run_flow_internal(start_runtime_id, "Exec")?;

        info!("Execution finished");
        self.logs.push("Execution finished".to_string());
        Ok(self.logs.clone())
    }

    /// 内部：执行流程
    fn run_flow_internal(&mut self, node_id: NodeId, output_exec_name: &str) -> Result<(), String> {
        // 检测循环执行
        if self.execution_stack.contains(&node_id) {
            let cycle_info = format!(
                "Cycle detected: execution stack = {:?}",
                self.execution_stack
            );
            info!("[ERROR] {}", cycle_info);
            self.logs.push(format!("[ERROR] {}", cycle_info));
            return Err(cycle_info);
        }

        // 将当前节点加入执行栈
        self.execution_stack.push(node_id);

        let node = self
            .nodes
            .get(&node_id)
            .ok_or_else(|| format!("Node not found: {:?}", node_id))?
            .clone();

        let node_guard = node.lock().unwrap();
        let node_type = node_guard.node_type().to_string();
        let node_name = node_guard.name().to_string();
        drop(node_guard);

        let log_msg = format!(">>> Executing Node: {} ({})", node_name, node_type);
        info!("{}", log_msg);
        self.logs.push(log_msg);

        // 从注册中心获取原型并执行
        let proto = crate::executor::node::registry::get_registry().get_prototype(&node_type);

        // 构造临时 NodeData 供处理器使用
        let node_data = {
            let node_guard = node.lock().unwrap();
            
            // 正确构造 inputs 和 outputs
            let mut inputs = Vec::new();
            let mut outputs = Vec::new();
            
            // 填充输入 pins
            for input_pin in node_guard.inputs().iter() {
                // 找到对应的前端 pin ID
                let frontend_pin_id = self.data_pin_id_to_runtime_pin_id
                    .iter()
                    .find(|(_, &runtime_id)| runtime_id == input_pin.id())
                    .map(|(frontend_id, _)| frontend_id.clone())
                    .unwrap_or_default();
                
                inputs.push(crate::executor::node::data::PinData {
                    id: frontend_pin_id,
                    name: input_pin.name().to_string(),
                    pin_type: input_pin.data_type().to_string(),
                    links: vec![],
                    default_value: None,
                    is_array: false,
                });
            }
            
            // 填充输出 pins
            for output_pin in node_guard.outputs().iter() {
                let frontend_pin_id = self.data_pin_id_to_runtime_pin_id
                    .iter()
                    .find(|(_, &runtime_id)| runtime_id == output_pin.id())
                    .map(|(frontend_id, _)| frontend_id.clone())
                    .unwrap_or_default();
                
                outputs.push(crate::executor::node::data::PinData {
                    id: frontend_pin_id,
                    name: output_pin.name().to_string(),
                    pin_type: output_pin.data_type().to_string(),
                    links: vec![],
                    default_value: None,
                    is_array: false,
                });
            }
            
            NodeData {
                id: self
                    .runtime_id_to_data_id
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_default(),
                node_type: node_guard.node_type().to_string(),
                title: node_guard.name().to_string(),
                inputs,
                outputs,
                variable_id: node_guard.variable_id(),
                sub_graph_id: None,
            }
        };

        // 实际执行节点逻辑
        let next_exec_name = if let Some(p) = proto {
            match p.process_flow(self, &node_data) {
                Ok(next) => {
                    let log_msg = format!("  -> Node returned next exec: '{}'", next);
                    info!("{}", log_msg);
                    self.logs.push(log_msg);
                    if next.is_empty() {
                        output_exec_name.to_string()
                    } else {
                        next
                    }
                }
                Err(e) => {
                    let err_msg = format!("[ERROR] Node execution failed: {}", e);
                    info!("{}", err_msg);
                    self.logs.push(err_msg);
                    return Err(e);
                }
            }
        } else {
            let log_msg = format!(
                "  -> No prototype found for '{}', using default flow",
                node_type
            );
            info!("{}", log_msg);
            self.logs.push(log_msg);
            output_exec_name.to_string()
        };

        // 如果是返回操作
        if next_exec_name == "__RETURN__" {
            self.execution_stack.pop();
            return Ok(());
        }

        if !next_exec_name.is_empty() {
            self.trigger_next_flow(node_id, &next_exec_name)?;
        }

        // 从执行栈移除当前节点
        self.execution_stack.pop();
        Ok(())
    }

    /// 触发下一个流程节点
    fn trigger_next_flow(&mut self, node_id: NodeId, pin_name: &str) -> Result<(), String> {
        let node = self
            .nodes
            .get(&node_id)
            .ok_or_else(|| format!("Node not found: {:?}", node_id))?
            .clone();

        let node_guard = node.lock().unwrap();

        info!(
            "[trigger_next_flow] Looking for pin '{}' in node '{}'",
            pin_name,
            node_guard.name()
        );

        // 查找执行 Pin（通过 name） - 先查找输出，再查找输入
        let pin_id = if let Some(pin) = node_guard.get_out_exec_pin_by_name(pin_name) {
            Some(pin.id())
        } else if let Some(pin) = node_guard.get_in_exec_pin_by_name(pin_name) {
            Some(pin.id())
        } else {
            None
        };

        drop(node_guard);

        if let Some(pin_id) = pin_id {
            info!("[trigger_next_flow] Found pin '{}'", pin_name);

            // 获取下游连接
            let downstream_pins = self.connection_manager.get_downstream(pin_id);
            
            if !downstream_pins.is_empty() {
                // 普通节点只执行第一个下游连接
                let next_pin_id = downstream_pins[0];
                let next_node_id = self
                    .pin_to_node
                    .get(&next_pin_id)
                    .ok_or("Target node not found")?;
                return self.run_flow_internal(*next_node_id, "Out");
            }
        } else {
            info!("[trigger_next_flow] Pin '{}' not found!", pin_name);
        }

        Ok(())
    }

    /// 记录图结构日志
    fn log_graph_structure(&mut self) {
        let mut messages = vec!["=== Execution Graph Structure ===".to_string()];

        for (node_id, node_arc) in &self.nodes {
            let node = node_arc.lock().unwrap();
            messages.push(format!(
                "Node: {} ({}) - ID: {:?}",
                node.name(),
                node.node_type(),
                node_id
            ));

            // 显示上游和下游节点
            let upstream_nodes = self.connection_manager.get_upstream_nodes(*node_id);
            let downstream_nodes = self.connection_manager.get_downstream_nodes(*node_id);

            if !upstream_nodes.is_empty() {
                messages.push(format!("  Upstream nodes: {:?}", upstream_nodes));
            }
            if !downstream_nodes.is_empty() {
                messages.push(format!("  Downstream nodes: {:?}", downstream_nodes));
            }
        }

        messages.push("===================================".to_string());

        for msg in messages {
            self.log(msg);
        }
    }
}

// 实现 ExecutionContextTrait (暂时保留接口兼容性)
impl ExecutionContextTrait for ExecutionContext {
    fn get_pin_value(&mut self, pin_id_str: &str) -> Value {
        // 1. 获取运行时 PinId
        let pin_id = match self.data_pin_id_to_runtime_pin_id.get(pin_id_str) {
            Some(&id) => id,
            None => {
                // self.log(format!("[WARN] Pin '{}' not found in runtime mapping", pin_id_str));
                return Value::Null;
            }
        };

        // 2. 查找上游连接 (Input -> Output)
        // get_upstream 返回 Option<PinId>
        let output_pin_id = match self.connection_manager.get_upstream(pin_id) {
            Some(id) => id,
            None => return Value::Null,
        };

        // 3. 找到输出节点
        let node_id = match self.pin_to_node.get(&output_pin_id) {
            Some(id) => *id,
            None => return Value::Null,
        };

        // 4. 获取节点并执行
        let node_arc = match self.nodes.get(&node_id) {
            Some(n) => n.clone(),
            None => return Value::Null,
        };

        let node_type = node_arc.lock().unwrap().node_type().to_string();

        // 构造 NodeData
        let node_data = {
            let node_guard = node_arc.lock().unwrap();
            NodeData {
                id: self
                    .runtime_id_to_data_id
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_default(),
                node_type: node_guard.node_type().to_string(),
                title: node_guard.name().to_string(),
                inputs: vec![],
                outputs: vec![],
                variable_id: node_guard.variable_id(),
                sub_graph_id: None,
            }
        };

        let proto = crate::executor::node::registry::get_registry().get_prototype(&node_type);

        if let Some(p) = proto {
            // 查找输出 Pin 的字符串 ID
            let output_pin_id_str = self
                .data_pin_id_to_runtime_pin_id
                .iter()
                .find(|(_, &v)| v == output_pin_id)
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            return p.process_data(self, &node_data, &output_pin_id_str);
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
        // 将 String node_id 转换为 NodeId
        if let Some(&runtime_id) = self.data_id_to_runtime_id.get(node_id) {
            self.trigger_next_flow(runtime_id, output_pin)
        } else {
            Err(format!("Node not found: {}", node_id))
        }
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

    fn find_node_by(&self, _predicate: &dyn Fn(&NodeData) -> bool) -> Option<String> {
        // TODO: 需要重新实现，因为我们现在使用 GenericNode 而不是 NodeData
        None
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

        // 同步创建窗口，使用简单配置避免问题
        match WebviewWindowBuilder::new(
            &app_handle,
            label.clone(),
            WebviewUrl::App(url.into()),
        )
        .title(title)
        .inner_size(800.0, 600.0)
        .min_inner_size(400.0, 300.0)
        .resizable(true)
        .visible(true)  // 直接设置为可见
        .decorations(false)  // 使用自定义标题栏
        .center()
        .build()
        {
            Ok(_window) => {
                let success_msg = format!("Window '{}' created successfully", label);
                info!("{}", success_msg);
                self.logs.push(success_msg);
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to create window '{}': {}", label, e);
                info!("[ERROR] {}", error_msg);
                self.logs.push(format!("[ERROR] {}", error_msg));
                Err(error_msg)
            }
        }
    }

    fn trigger_flow(&mut self, node_id: &str, output_pin: &str) -> Result<(), String> {
        // 将 String node_id 转换为 NodeId
        if let Some(&runtime_id) = self.data_id_to_runtime_id.get(node_id) {
            self.trigger_next_flow(runtime_id, output_pin)
        } else {
            Err(format!("Node not found: {}", node_id))
        }
    }

    /// 执行指定pin的所有下游连接（供节点内部使用）
    fn execute_pin_downstream(&mut self, node_id: &str, pin_name: &str) -> Result<(), String> {
        // 将 String node_id 转换为 NodeId
        let runtime_id = self.data_id_to_runtime_id.get(node_id)
            .ok_or_else(|| format!("Node not found: {}", node_id))?;

        let node = self
            .nodes
            .get(runtime_id)
            .ok_or_else(|| format!("Node not found: {:?}", runtime_id))?
            .clone();

        let node_guard = node.lock().unwrap();

        // 查找执行 Pin（通过 name）
        let pin_id = if let Some(pin) = node_guard.get_out_exec_pin_by_name(pin_name) {
            Some(pin.id())
        } else if let Some(pin) = node_guard.get_in_exec_pin_by_name(pin_name) {
            Some(pin.id())
        } else {
            None
        };

        drop(node_guard);

        if let Some(pin_id) = pin_id {
            // 获取下游连接
            let downstream_pins = self.connection_manager.get_downstream(pin_id);
            
            info!("[execute_pin_downstream] Executing {} downstream connections for pin '{}'", downstream_pins.len(), pin_name);
            
            // 执行所有下游连接
            for (index, &next_pin_id) in downstream_pins.iter().enumerate() {
                let next_node_id = self
                    .pin_to_node
                    .get(&next_pin_id)
                    .ok_or("Target node not found")?;
                
                info!("[execute_pin_downstream] Executing downstream node #{}: {:?}", index + 1, next_node_id);
                self.run_flow_internal(*next_node_id, "Out")?;
            }
        } else {
            info!("[execute_pin_downstream] Pin '{}' not found!", pin_name);
        }

        Ok(())
    }

    fn open_window_async(&mut self, label: String, title: String, url: String) -> Result<(), String> {
        let app_handle = self
            .app_handle
            .as_ref()
            .ok_or("AppHandle not available in execution context")?
            .clone();

        let log_msg = format!("Opening window async: {} ({})", title, url);
        info!("{}", log_msg);
        self.logs.push(log_msg.clone());

        // 检查窗口是否已经存在
        if let Some(existing_window) = app_handle.get_webview_window(&label) {
            if let Err(e) = existing_window.set_focus() {
                info!("[WARN] Failed to focus existing window '{}': {}", label, e);
            }
            let success_msg = format!("Window '{}' already exists, focusing", label);
            info!("{}", success_msg);
            self.logs.push(success_msg);
            return Ok(());
        }

        // 在新线程中异步创建窗口，不阻塞主线程
        let label_clone = label.clone();
        let title_clone = title.clone();
        let url_clone = url.clone();

        std::thread::spawn(move || {
            // 添加小延迟确保主线程不被阻塞
            std::thread::sleep(std::time::Duration::from_millis(50));
            
            match WebviewWindowBuilder::new(
                &app_handle,
                label_clone.clone(),
                WebviewUrl::App(url_clone.into()),
            )
            .title(title_clone)
            .inner_size(800.0, 600.0)
            .min_inner_size(400.0, 300.0)
            .resizable(true)
            .visible(false)  // 先创建为不可见
            .decorations(false)  // 使用自定义标题栏
            .transparent(false)
            .center()
            .build()
            {
                Ok(window) => {
                    info!("Window '{}' created successfully (async)", label_clone);
                    // 创建成功后显示窗口
                    if let Err(e) = window.show() {
                        info!("[ERROR] Failed to show window '{}': {}", label_clone, e);
                    } else {
                        info!("Window '{}' shown successfully", label_clone);
                    }
                }
                Err(e) => {
                    info!("[ERROR] Failed to create window '{}' (async): {}", label_clone, e);
                }
            }
        });

        let success_msg = format!("Window '{}' creation initiated (async)", label);
        info!("{}", success_msg);
        self.logs.push(success_msg);

        Ok(())
    }

    fn trigger_flow_by_pin(&mut self, node_id: &str, pin_name: &str) -> Result<(), String> {
        self.execute_pin_downstream(node_id, pin_name)
    }
}
