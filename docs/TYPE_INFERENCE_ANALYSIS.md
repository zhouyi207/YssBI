# 类型推断时机分析

## 📊 总结

**当前系统采用的是「执行前验证」模式，在连接创建时进行类型推断和验证。**

---

## 🔍 详细分析

### 1. 类型推断的触发时机

#### ✅ **连接创建时（执行前验证）**

```rust
// src-tauri/src/graph/core/graph_data.rs:308-318
pub fn connect(&self, from_pin: PinId, to_pin: PinId) -> Result<(), String> {
    // ... 检查 Pin 是否存在 ...
    
    // 只对有类型描述的 Pin（Data Pin）进行类型推断
    // Exec Pin 没有类型描述，不需要类型推断
    let from_pin_instance = pins.get(&from_pin).unwrap();
    let to_pin_instance = pins.get(&to_pin).unwrap();

    if from_pin_instance.definition.type_desc.is_some()
        && to_pin_instance.definition.type_desc.is_some()
    {
        // 🔥 关键：在连接创建时立即进行类型推断
        self.type_inference
            .write()
            .unwrap()
            .infer_connection(from_pin, to_pin)?;  // ⚠️ 如果类型不匹配，这里会返回 Err
    }

    self.connections.connect(from_pin, to_pin)  // 只有类型推断成功才会执行到这里
}
```

**特点：**
- ✅ 在用户拖拽连接时立即验证
- ✅ 类型不匹配会立即返回错误，阻止连接创建
- ✅ 保证图中所有连接都是类型安全的

---

#### ✅ **节点创建时（注册类型变量）**

```rust
// src-tauri/src/graph/core/graph_data.rs:92-130
pub fn create_node(&self, node_type: &str) -> Result<NodeId, String> {
    // ...
    
    // 🔥 为每个节点实例创建新的类型变量 ID 映射
    let mut type_var_map: HashMap<TypeVarId, TypeVarId> = HashMap::new();

    // 注册类型变量到类型推断系统（为每个实例生成新的 ID）
    {
        let mut ti = self.type_inference.write().unwrap();
        for type_var in &definition.type_vars {
            let new_id = TypeVarId::new();
            type_var_map.insert(type_var.id, new_id);

            let mut new_type_var = type_var.clone();
            new_type_var.id = new_id;
            ti.register_type_var(new_type_var);  // 注册类型变量
        }
    }

    // 创建 Pin 并注册到类型推断系统
    for pin_def in &definition.pins {
        // ...
        if let Some(type_desc) = &pin.definition.type_desc {
            // 重新映射类型变量 ID
            let mut remapped_type_desc = type_desc.clone();
            // ...
            
            self.type_inference
                .write()
                .unwrap()
                .register_pin(pin_id, remapped_type_desc);  // 注册 Pin 类型
        }
    }
}
```

**特点：**
- 为每个节点实例创建独立的类型变量
- 注册所有 Pin 的类型描述
- 为后续的类型推断做准备

---

#### ✅ **节点删除时（重建类型推断上下文）**

```rust
// src-tauri/src/graph/core/graph_data.rs:151
pub fn remove_node(&self, node_id: NodeId) -> Result<(), String> {
    // ... 删除节点和 Pin ...
    
    self.rebuild_type_inference();  // 🔥 重建整个类型推断上下文
    
    Ok(())
}

// src-tauri/src/graph/core/graph_data.rs:168-196
fn rebuild_type_inference(&self) {
    let mut ti = self.type_inference.write().unwrap();
    ti.clear();

    // 重新注册所有节点的类型变量
    let nodes = self.nodes.read().unwrap();
    for node in nodes.values() {
        for type_var in &node.definition.type_vars {
            ti.register_type_var(type_var.clone());
        }
    }

    // 重新注册所有 Pin
    let pins = self.pins.read().unwrap();
    for pin in pins.values() {
        if let Some(type_desc) = &pin.definition.type_desc {
            ti.register_pin(pin.id, type_desc.clone());
        }
    }

    // 🔥 重新推断所有连接
    for conn in self.connections.all_connections() {
        let _ = ti.infer_connection(conn.from_pin, conn.to_pin);
    }
}
```

**特点：**
- 删除节点后重新验证所有连接
- 确保图的类型一致性

---

### 2. 执行时的类型使用（非验证）

#### ❌ **执行时不进行类型验证**

```rust
// src-tauri/src/execution/engine/executor.rs:91-117
fn execute_node(&mut self, frame: &ExecutionFrame) -> Result<ExecutionEffect, String> {
    let node_id = frame.node_id;
    
    // 在执行节点之前，先执行所有上游的纯数据节点
    self.execute_upstream_data_nodes(node_id)?;

    // 创建执行上下文
    let mut ctx = GraphNodeExecutionContext::new(node_id, self.graph.clone());

    // 🔥 执行节点的 FlowProcessor（如果有）
    let result = if let Some(ref processor) = definition.flow_processor {
        processor(&mut ctx)  // ⚠️ 这里不进行类型验证，直接执行
    } else {
        // 执行节点的 DataEvaluator（如果有）
        if let Some(ref data_evaluator) = definition.data_evaluator {
            data_evaluator(&mut ctx)?;  // ⚠️ 这里也不进行类型验证
        }
        Ok(ExecutionEffect::Done)
    };
    
    // ...
}
```

**特点：**
- ❌ 执行时不进行类型验证
- ✅ 只是查询类型信息（通过 `get_pin_type_by_role`）
- ✅ 假设所有连接都已经通过了类型检查

---

#### ✅ **执行时查询类型信息（用于运行时决策）**

```rust
// src-tauri/src/execution/context/node_execution_context.rs:40-46
pub trait NodeExecutionContext {
    // ...
    
    /// 用于在运行时获取类型推断的结果
    /// 返回 None 表示类型变量未绑定
    fn get_bound_type(&self, type_var_id: TypeVarId) -> Option<DataType>;

    /// 通过角色获取 Pin 的推断类型
    /// 用于在运行时获取 Pin 的实际类型（经过类型推断后）
    fn get_pin_type_by_role(&self, role: &PinRole) -> Result<DataType, String>;
}
```

**用途：**
- 获取类型变量的绑定结果
- 用于泛型节点的运行时决策
- 例如：`Array<T>` 节点需要知道 `T` 的具体类型

---

## 🎯 类型推断流程图

```
┌─────────────────────────────────────────────────────────────┐
│                    用户操作：创建连接                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  GraphData.connect(from_pin, to_pin)                        │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ 1. 检查 Pin 是否存在                                   │  │
│  │ 2. 检查是否为 Data Pin（有 type_desc）                │  │
│  │ 3. 调用 type_inference.infer_connection()             │  │
│  │    ├─ 获取两个 Pin 的类型描述                         │  │
│  │    ├─ 调用 unify() 进行类型统一                       │  │
│  │    │   ├─ Concrete vs Concrete: 检查兼容性            │  │
│  │    │   ├─ TypeVar vs Concrete: 绑定类型变量           │  │
│  │    │   └─ TypeVar vs TypeVar: 延迟绑定                │  │
│  │    └─ 如果类型不匹配，返回 Err ❌                      │  │
│  │ 4. 只有类型推断成功，才创建连接 ✅                     │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              连接创建成功，类型信息已绑定                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    用户操作：执行图                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Executor.execute_node(node_id)                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ 1. 执行上游数据节点                                    │  │
│  │ 2. 创建执行上下文                                      │  │
│  │ 3. 调用 FlowProcessor / DataEvaluator                 │  │
│  │    ├─ 通过 ctx.get_input() 获取输入值                 │  │
│  │    ├─ 通过 ctx.get_pin_type_by_role() 查询类型        │  │
│  │    └─ ⚠️ 不进行类型验证，假设类型已正确               │  │
│  │ 4. 返回执行效果                                        │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 📋 类型推断系统的关键组件

### 1. TypeInferenceContext

```rust
pub struct TypeInferenceContext {
    /// 类型变量定义（来自 Graph / NodeDefinition）
    type_vars: HashMap<TypeVarId, TypeVarDefinition>,

    /// 推断过程中的临时绑定
    bindings: HashMap<TypeVarId, DataType>,

    /// Pin 到类型描述的映射
    pin_types: HashMap<PinId, PinTypeDesc>,
}
```

**职责：**
- 管理类型变量和绑定
- 执行类型统一（unification）
- 解析 Pin 的最终类型

---

### 2. PinTypeDesc

```rust
pub struct PinTypeDesc {
    /// 数据类型
    pub data_type: PinDataType,  // Concrete | TypeVar | Unknown

    /// 是否可选
    pub is_optional: bool,

    /// 是否数组
    pub is_array: bool,
}
```

**职责：**
- 描述 Pin 的类型信息
- 支持具体类型、类型变量、未知类型

---

### 3. 类型统一算法

```rust
fn unify(&mut self, a: &PinTypeDesc, b: &PinTypeDesc) -> Result<(), String> {
    match (ta, tb) {
        // 具体类型 vs 具体类型：检查兼容性
        (Concrete(a), Concrete(b)) => {
            if compatible(a, b) { Ok(()) } else { Err(...) }
        }

        // 类型变量 vs 具体类型：绑定类型变量
        (TypeVar(var), Concrete(vt)) => {
            check_constraints(var, vt)?;
            bind(var, vt)
        }

        // 类型变量 vs 类型变量：延迟绑定
        (TypeVar(a), TypeVar(b)) => {
            if a == b { Ok(()) } else { Ok(()) }  // 可选：union
        }

        // 未知类型：总是兼容
        (Unknown, _) | (_, Unknown) => Ok(())
    }
}
```

---

## ✅ 优点

1. **早期错误检测**
   - 在连接创建时立即发现类型错误
   - 用户可以立即修正，不需要等到执行时

2. **执行时性能**
   - 执行时不需要类型检查，性能更好
   - 假设所有连接都是类型安全的

3. **类型安全保证**
   - 图中所有连接都经过类型验证
   - 不会出现运行时类型错误

4. **支持泛型**
   - 通过类型变量支持泛型节点
   - 类型推断自动绑定类型变量

---

## ⚠️ 潜在问题

1. **动态类型变化**
   - 如果 Pin 的类型在运行时动态变化（例如 DataFrame schema），当前系统无法处理
   - 建议：将 schema 信息与类型系统分离

2. **类型推断的完整性**
   - 删除节点后会重建类型推断上下文，但可能有性能问题
   - 建议：增量更新类型推断

3. **错误提示**
   - 类型不匹配时的错误信息可能不够友好
   - 建议：提供更详细的类型错误信息

---

## 🎓 与其他系统的对比

### TypeScript（执行前验证）
```typescript
// 编译时类型检查
let x: number = "hello";  // ❌ 编译错误
```

### Python（执行时验证）
```python
# 运行时类型检查
def add(a: int, b: int) -> int:
    return a + b

add("hello", "world")  # ❌ 运行时错误（如果使用 type hints + runtime checker）
```

### 当前系统（执行前验证）
```rust
// 连接创建时类型检查
graph.connect(string_output, int_input)  // ❌ 立即返回错误
```

---

## 📝 结论

**当前系统采用「执行前验证」模式，在连接创建时进行类型推断和验证。**

这种设计：
- ✅ 提供了早期错误检测
- ✅ 保证了执行时的类型安全
- ✅ 提高了执行时性能
- ⚠️ 但需要注意动态类型变化的场景

总体来说，这是一个合理的设计选择，符合可视化编程工具的最佳实践（如 Unreal Blueprint、Unity Visual Scripting）。
