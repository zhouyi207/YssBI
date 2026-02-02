# 动态 Pin 和 Pin 默认值实现状态

## ✅ 已完成（2026-02-02）

### 1. 数据结构扩展
- ✅ `PinDto` 添加了 `user_value`, `show_widget`, `widget_type` 字段
- ✅ `SerializedPin` 添加了 `user_value` 字段
- ✅ `SerializedNode` 添加了 `dynamic_pins` 字段
- ✅ 新增 `DynamicPinMetadata` 结构

### 2. 后端命令实现
- ✅ `update_pin_user_value` - 更新 Pin 用户值
- ✅ `clear_pin_user_value` - 清除 Pin 用户值
- ✅ `add_dynamic_pin` - 添加动态 Pin（完整实现）
- ✅ `remove_dynamic_pin` - 移除动态 Pin（完整实现）
- ✅ 所有命令已在 `lib.rs` 中注册

### 3. 注册表改进
- ✅ `NodeRegistry::get_node()` 方法添加

### 4. 编译错误修复
- ✅ 修复了 `executor/context.rs` 中的 PinDto 初始化（2处）
- ✅ 修复了 `project/io.rs` 中的 SerializedPin 和 SerializedNode 初始化
- ✅ 修复了 `commands/execution.rs` 中的 PinDto 初始化
- ✅ 修复了 `commands/nodes.rs` 中的 Option `?` 操作符错误
- ✅ 移除了 `dynamic_add.rs` 中未使用的导入
- ✅ **编译成功** - `cargo check` 通过（仅有静态变量警告）

### 5. 文档
- ✅ 动态 Pin 持久化设计文档
- ✅ 动态 Pin 流程图
- ✅ Pin 默认值系统设计
- ✅ Pin 值快速参考
- ✅ 前端集成指南
- ✅ 快速开始指南

## ⚠️ 待完成

### 1. 执行逻辑更新（高优先级）

需要在 `GenericInDataPin` 中添加用户值支持：

```rust
pub struct GenericInDataPin {
    id: PinId,
    node_id: NodeId,
    name: String,
    type_desc: PinTypeDesc,
    state: RwLock<DataPinState>,
    value: RwLock<DataValue>,
    upstream: RwLock<Option<PinId>>,
    listeners: Mutex<Vec<Box<dyn Fn(DataPinEvent) + Send + Sync + 'static>>>,
    
    // 🆕 添加用户值和默认值支持
    user_value: RwLock<Option<Value>>,
    default_value: Option<Value>,
}

impl GenericInDataPin {
    pub fn new(node_id: NodeId, name: impl Into<String>, type_desc: PinTypeDesc) -> Self {
        Self {
            // ... 现有字段
            user_value: RwLock::new(None),
            default_value: None,
        }
    }
    
    pub fn set_user_value(&self, value: Option<Value>) {
        *self.user_value.write().unwrap() = value;
    }
    
    pub fn get_user_value(&self) -> Option<Value> {
        self.user_value.read().unwrap().clone()
    }
    
    pub fn set_default_value(&mut self, value: Option<Value>) {
        self.default_value = value;
    }
    
    pub fn get_default_value(&self) -> Option<Value> {
        self.default_value.clone()
    }
}
```

### 2. ExecutionContext 中实现三层优先级

在 `executor/context.rs` 的 `get_pin_value` 方法中实现：

```rust
fn get_pin_value(&mut self, pin_id_str: &str) -> Value {
    // 1. 获取运行时 PinId
    let pin_id = match self.data_pin_id_to_runtime_pin_id.get(pin_id_str) {
        Some(&id) => id,
        None => return Value::Null,
    };

    // 2. 查找上游连接（优先级最高）
    if let Some(output_pin_id) = self.connection_manager.get_upstream(pin_id) {
        // 检查缓存
        if let Some(cached_value) = self.data_cache.get(&output_pin_id) {
            return cached_value.clone();
        }
        
        // 计算连接值（现有逻辑）
        // ...
    }
    
    // 3. 如果没有连接，尝试获取用户值（优先级中）
    if let Some(node_id) = self.pin_to_node.get(&pin_id) {
        if let Some(node_arc) = self.nodes.get(node_id) {
            let node_guard = node_arc.lock().unwrap();
            
            // 查找输入 Pin
            for input_pin in node_guard.inputs().iter() {
                if input_pin.id() == pin_id {
                    // 尝试获取用户值
                    if let Some(user_val) = input_pin.get_user_value() {
                        return user_val;
                    }
                    
                    // 最后尝试默认值（优先级最低）
                    if let Some(default_val) = input_pin.get_default_value() {
                        return default_val;
                    }
                    
                    break;
                }
            }
        }
    }
    
    Value::Null
}
```

### 3. 节点创建时加载用户值

在 `executor/context.rs` 的 `create_node_from_data` 方法中：

```rust
// 创建输入 Pin
for pin_data in &node_data.inputs {
    if pin_data.pin_type == "exec" {
        // ... exec pin 逻辑
    } else {
        use crate::executor::pin::GenericInDataPin;
        use crate::executor::value::{ValueType, PinTypeDesc};
        let mut pin = GenericInDataPin::new(
            runtime_id, 
            &pin_data.name, 
            PinTypeDesc::concrete(ValueType::from_string(&pin_data.pin_type))
        );
        
        // 🆕 设置默认值和用户值
        pin.set_default_value(pin_data.default_value.clone());
        if let Some(user_val) = &pin_data.user_value {
            pin.set_user_value(Some(user_val.clone()));
        }
        
        let pin_id = pin.id();
        node.add_in_data_pin(pin);
        // ...
    }
}
```

### 4. 测试计划

#### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_user_value() {
        let pin = GenericInDataPin::new(
            uuid::Uuid::new_v4(),
            "test_pin",
            PinTypeDesc::concrete(ValueType::Int)
        );
        
        // 测试设置和获取用户值
        pin.set_user_value(Some(json!(42)));
        assert_eq!(pin.get_user_value(), Some(json!(42)));
        
        // 测试清除用户值
        pin.set_user_value(None);
        assert_eq!(pin.get_user_value(), None);
    }

    #[test]
    fn test_pin_value_priority() {
        // 创建测试图
        // 1. 测试连接值优先
        // 2. 测试用户值次之
        // 3. 测试默认值最后
    }

    #[test]
    fn test_dynamic_pin_persistence() {
        // 测试动态 Pin 的序列化和反序列化
    }
}
```

#### 集成测试

1. **Pin 用户值测试**
   - 创建节点
   - 设置 Pin 用户值
   - 保存项目
   - 加载项目
   - 验证用户值恢复

2. **动态 Pin 测试**
   - 创建支持动态 Pin 的节点
   - 添加动态 Pin
   - 保存项目
   - 加载项目
   - 验证动态 Pin 恢复

3. **值优先级测试**
   - 创建两个节点并连接
   - 设置目标 Pin 的用户值
   - 执行图
   - 验证使用连接值而非用户值

## 📋 实现检查清单

### 后端核心功能
- [x] 数据结构定义
- [x] 命令实现
- [x] 命令注册
- [x] 编译错误修复
- [ ] Pin 实现更新（添加 user_value 字段）
- [ ] ExecutionContext 更新（三层优先级）
- [ ] 节点创建时加载值
- [ ] 单元测试
- [ ] 集成测试

### 前端集成（待前端开发）
- [ ] Pin 输入控件 UI
- [ ] 动态 Pin 管理 UI
- [ ] API 调用集成
- [ ] 用户交互测试

## 🎯 下一步行动

1. **立即执行**：更新 `GenericInDataPin` 添加 `user_value` 和 `default_value` 字段
2. **然后**：更新 `ExecutionContext::get_pin_value` 实现三层优先级
3. **接着**：更新 `create_node_from_data` 加载用户值
4. **最后**：编写测试验证功能

## 📚 相关文档

- [动态 Pin 持久化设计](./architecture/DYNAMIC_PIN_PERSISTENCE.md)
- [动态 Pin 流程图](./architecture/DYNAMIC_PIN_FLOW.md)
- [Pin 默认值系统设计](./architecture/PIN_DEFAULT_VALUE_DESIGN.md)
- [Pin 值快速参考](./architecture/PIN_VALUE_QUICK_REFERENCE.md)
- [前端集成指南](./examples/DYNAMIC_PIN_FRONTEND_GUIDE.md)
- [快速开始](./examples/DYNAMIC_PIN_QUICKSTART.md)

## 🔧 编译状态

```
✅ cargo check 通过
⚠️  7 个警告（静态变量引用，非关键）
✅ 所有结构体初始化已修复
✅ 所有命令已注册
```

### 1. 修复编译错误

需要修复以下文件中的结构体初始化：

**错误类型 1：PinDto 初始化缺少字段**
- 位置：`executor/context.rs` 和其他文件
- 修复：添加缺失的字段

```rust
// 修复前
PinDto {
    id: frontend_pin_id,
    name: input_pin.name().to_string(),
    pin_type: input_pin.data_type().to_string(),
    links: vec![],
    default_value: None,
    is_array: false,
}

// 修复后
PinDto {
    id: frontend_pin_id,
    name: input_pin.name().to_string(),
    pin_type: input_pin.data_type().to_string(),
    links: vec![],
    default_value: None,
    user_value: None,           // 🆕
    is_array: false,
    show_widget: true,          // 🆕
    widget_type: None,          // 🆕
}
```

**错误类型 2：SerializedPin 初始化缺少字段**
- 位置：`project/io.rs`
- 修复：添加 `user_value: None`

```rust
// 修复前
SerializedPin {
    id,
    name,
    pin_type,
    links,
    default_value,
    is_array,
}

// 修复后
SerializedPin {
    id,
    name,
    pin_type,
    links,
    default_value,
    user_value: None,  // 🆕
    is_array,
}
```

**错误类型 3：SerializedNode 初始化缺少字段**
- 位置：`project/io.rs`, `state/node_crud.rs`
- 修复：添加 `dynamic_pins: None`

```rust
// 修复前
SerializedNode {
    id,
    node_type,
    title,
    position,
    // ...
    inputs,
    outputs,
}

// 修复后
SerializedNode {
    id,
    node_type,
    title,
    position,
    // ...
    inputs,
    outputs,
    dynamic_pins: None,  // 🆕
}
```

**错误类型 4：Option 的 ? 操作符**
- 位置：`commands/nodes.rs`
- 修复：使用 `.ok_or(...)?`

```rust
// 修复前
let prototype = registry.get_node(&node.node_type)?;

// 修复后
let prototype = registry.get_node(&node.node_type)
    .ok_or_else(|| format!("Node type '{}' not found", node.node_type))?;
```

### 2. 批量修复脚本

可以使用以下命令查找所有需要修复的位置：

```bash
# 查找 PinDto 初始化
rg "PinDto\s*\{" src-tauri/src --type rust

# 查找 SerializedPin 初始化
rg "SerializedPin\s*\{" src-tauri/src --type rust

# 查找 SerializedNode 初始化
rg "SerializedNode\s*\{" src-tauri/src --type rust
```

或者运行提供的 Python 脚本（需要完善）：
```bash
python src-tauri/fix_dto_init.py
```

### 3. 执行逻辑更新

需要在 `executor/context.rs` 中实现 Pin 值的三层优先级：

```rust
impl ExecutionContextTrait for ExecutionContext {
    fn get_pin_value(&mut self, pin_id: &str) -> Value {
        // 1. 查找运行时 Pin
        let runtime_pin_id = self.data_pin_id_to_runtime_pin_id
            .get(pin_id)
            .copied();
        
        if let Some(runtime_id) = runtime_pin_id {
            if let Some(node_id) = self.pin_to_node.get(&runtime_id) {
                if let Some(node) = self.nodes.get(node_id) {
                    let node_guard = node.lock().unwrap();
                    
                    if let Some(input_pin) = node_guard.get_input_concrete(&runtime_id) {
                        // 🔑 使用三层优先级
                        // 1. 连接值
                        if let Some(connected_id) = input_pin.connected_pin() {
                            return self.get_pin_value(&connected_id.to_string());
                        }
                        
                        // 2. 用户值
                        if let Some(user_val) = input_pin.get_user_value() {
                            return user_val;
                        }
                        
                        // 3. 默认值
                        if let Some(default_val) = input_pin.get_default_value() {
                            return default_val;
                        }
                    }
                }
            }
        }
        
        Value::Null
    }
}
```

### 4. Pin 实现更新

需要在 `GenericInDataPin` 中添加用户值支持：

```rust
pub struct GenericInDataPin {
    id: PinId,
    name: String,
    data_type: PinTypeDesc,
    default_value: Option<Value>,
    user_value: RwLock<Option<Value>>,  // 🆕
    // ...
}

impl GenericInDataPin {
    pub fn set_user_value(&self, value: Option<Value>) {
        *self.user_value.write().unwrap() = value;
    }
    
    pub fn get_user_value(&self) -> Option<Value> {
        self.user_value.read().unwrap().clone()
    }
    
    pub fn get_default_value(&self) -> Option<Value> {
        self.default_value.clone()
    }
}
```

## 快速修复步骤

### 步骤 1：修复 executor/context.rs

查找所有 `PinDto {` 并添加缺失字段：
```rust
user_value: None,
show_widget: true,
widget_type: None,
```

### 步骤 2：修复 project/io.rs

查找所有 `SerializedPin {` 并添加：
```rust
user_value: None,
```

查找所有 `SerializedNode {` 并添加：
```rust
dynamic_pins: None,
```

### 步骤 3：修复 state/node_crud.rs

同样添加 `dynamic_pins: None` 到 `SerializedNode` 初始化。

### 步骤 4：验证编译

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

### 步骤 5：运行测试

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## 测试计划

### 单元测试

```rust
#[test]
fn test_pin_user_value() {
    // 测试用户值设置和获取
}

#[test]
fn test_pin_value_priority() {
    // 测试值优先级：连接 > 用户 > 默认
}

#[test]
fn test_dynamic_pin_add_remove() {
    // 测试动态 Pin 添加和移除
}

#[test]
fn test_dynamic_pin_persistence() {
    // 测试动态 Pin 持久化
}
```

### 集成测试

1. 创建节点并设置 Pin 用户值
2. 保存项目
3. 加载项目
4. 验证用户值恢复
5. 添加动态 Pin
6. 保存并重新加载
7. 验证动态 Pin 恢复

## 前端集成

前端需要实现的功能：

1. **Pin 输入控件**
   - 数字输入框
   - 文本输入框
   - 复选框
   - 颜色选择器
   - 滑块

2. **动态 Pin UI**
   - 添加 Pin 按钮
   - 移除 Pin 按钮
   - Pin 数量显示

3. **API 调用**
   ```typescript
   // Pin 值管理
   await invoke('update_pin_user_value', { ... });
   await invoke('clear_pin_user_value', { ... });
   
   // 动态 Pin 管理
   await invoke('add_dynamic_pin', { ... });
   await invoke('remove_dynamic_pin', { ... });
   ```

## 相关文档

- [动态 Pin 持久化设计](./architecture/DYNAMIC_PIN_PERSISTENCE.md)
- [动态 Pin 流程图](./architecture/DYNAMIC_PIN_FLOW.md)
- [Pin 默认值系统设计](./architecture/PIN_DEFAULT_VALUE_DESIGN.md)
- [Pin 值快速参考](./architecture/PIN_VALUE_QUICK_REFERENCE.md)
- [前端集成指南](./examples/DYNAMIC_PIN_FRONTEND_GUIDE.md)
- [快速开始](./examples/DYNAMIC_PIN_QUICKSTART.md)
