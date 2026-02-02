# 类型推断系统设计

## 设计目标

1. **动态类型推断**：Print 节点的 input 可以接受任意类型，类型由连接的 output 决定
2. **类型变量**：多个 Pin 共享同一个类型变量，一个确定后其他自动确定
3. **类型约束**：支持类型约束（如 Comparable, Numeric）
4. **向后兼容**：不破坏现有的 `ValueType` 系统

## 分层结构

### 第 1 层：基础类型（已有）

```rust
// src-tauri/src/executor/value/types.rs
pub enum ValueType {
    Null,
    Boolean,
    Int64,
    Float64,
    String,
    Date,
    Datetime,
    Duration,
    List(Box<ValueType>),
    Struct(Vec<(String, ValueType)>),
    DataFrame,
    Series,
    Any,  // 现有的泛型类型
}
```

**用途**：
- 运行时的实际类型
- 类型检查和转换
- 与 Polars 互操作

### 第 2 层：类型描述（新增）

```rust
// src-tauri/src/executor/value/type_desc.rs
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DataType {
    /// 具体类型（已知）
    Concrete(ValueType),
    
    /// 类型变量（待推断）
    TypeVar(TypeVarId),
    
    /// 未知类型（尚未连接）
    Unknown,
    
    /// 联合类型（可以是多种类型之一）
    Union(Vec<DataType>),
}
```

**关键区别**：
- `Unknown`：Pin 还没有连接，类型完全未知
- `TypeVar(T1)`：Pin 有类型变量，等待推断
- `Concrete(ValueType)`：类型已确定

**示例**：
```rust
// Print 节点的 Value input
DataType::TypeVar(T1)  // 等待推断

// 连接到 Divide 的 output 后
DataType::Concrete(ValueType::Float64)  // 推断为 Float64
```

### 第 3 层：类型变量（新增）

```rust
// src-tauri/src/executor/value/type_var.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeVarId(u32);

impl TypeVarId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        TypeVarId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}
```

**用途**：
- 表示待推断的类型
- 多个 Pin 可以共享同一个 TypeVarId
- 类型推断时统一解析

**示例**：
```rust
// Add 节点
A: TypeVar(T1)
B: TypeVar(T1)
Result: TypeVar(T1)

// 一旦 A 连接到 Number
T1 = Float64
// B 和 Result 自动变成 Float64
```

### 第 4 层：类型约束（新增）

```rust
// src-tauri/src/executor/value/type_constraint.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeConstraint {
    /// 必须是数字类型（Int64 或 Float64）
    Numeric,
    
    /// 必须是可比较类型（支持 <, >, ==）
    Comparable,
    
    /// 必须是可迭代类型（List, DataFrame, Series）
    Iterable,
    
    /// 必须是可序列化类型（可以转换为 JSON）
    Serializable,
    
    /// 必须是特定类型之一
    OneOf(Vec<ValueType>),
    
    /// 自定义约束（用于扩展）
    Custom(String),
}

impl TypeConstraint {
    /// 检查类型是否满足约束
    pub fn is_satisfied_by(&self, vtype: &ValueType) -> bool {
        match self {
            TypeConstraint::Numeric => matches!(vtype, ValueType::Int64 | ValueType::Float64),
            TypeConstraint::Comparable => matches!(
                vtype,
                ValueType::Int64 | ValueType::Float64 | ValueType::String | ValueType::Date | ValueType::Datetime
            ),
            TypeConstraint::Iterable => matches!(
                vtype,
                ValueType::List(_) | ValueType::DataFrame | ValueType::Series
            ),
            TypeConstraint::Serializable => !matches!(vtype, ValueType::DataFrame | ValueType::Series),
            TypeConstraint::OneOf(types) => types.contains(vtype),
            TypeConstraint::Custom(_) => true,  // 自定义约束需要额外处理
        }
    }
}
```

**用途**：
- 限制类型变量的可能取值
- 提供更好的类型错误提示
- 支持泛型节点（如 Compare, Filter）

### 第 5 层：Pin 类型描述（新增）

```rust
// src-tauri/src/executor/value/pin_type.rs
#[derive(Clone, Debug)]
pub struct PinTypeDesc {
    /// 类型描述
    pub data_type: DataType,
    
    /// 类型约束
    pub constraints: Vec<TypeConstraint>,
    
    /// 是否可选（允许 Null）
    pub optional: bool,
    
    /// 是否是数组
    pub is_array: bool,
}

impl PinTypeDesc {
    /// 创建具体类型的 Pin
    pub fn concrete(vtype: ValueType) -> Self {
        Self {
            data_type: DataType::Concrete(vtype),
            constraints: vec![],
            optional: false,
            is_array: false,
        }
    }
    
    /// 创建类型变量的 Pin
    pub fn type_var(var_id: TypeVarId) -> Self {
        Self {
            data_type: DataType::TypeVar(var_id),
            constraints: vec![],
            optional: false,
            is_array: false,
        }
    }
    
    /// 创建带约束的类型变量 Pin
    pub fn type_var_with_constraints(var_id: TypeVarId, constraints: Vec<TypeConstraint>) -> Self {
        Self {
            data_type: DataType::TypeVar(var_id),
            constraints,
            optional: false,
            is_array: false,
        }
    }
    
    /// 创建未知类型的 Pin（如 Print 的 input）
    pub fn unknown() -> Self {
        Self {
            data_type: DataType::Unknown,
            constraints: vec![],
            optional: false,
            is_array: false,
        }
    }
    
    /// 创建 Any 类型的 Pin（接受任意类型）
    pub fn any() -> Self {
        Self {
            data_type: DataType::Concrete(ValueType::Any),
            constraints: vec![],
            optional: false,
            is_array: false,
        }
    }
}
```

### 第 6 层：类型推断引擎（新增）

```rust
// src-tauri/src/executor/value/type_inference.rs
use std::collections::HashMap;

pub struct TypeInferenceContext {
    /// 类型变量的解析结果
    type_var_bindings: HashMap<TypeVarId, ValueType>,
    
    /// Pin 的类型描述
    pin_types: HashMap<PinId, PinTypeDesc>,
}

impl TypeInferenceContext {
    pub fn new() -> Self {
        Self {
            type_var_bindings: HashMap::new(),
            pin_types: HashMap::new(),
        }
    }
    
    /// 注册 Pin 的类型描述
    pub fn register_pin(&mut self, pin_id: PinId, type_desc: PinTypeDesc) {
        self.pin_types.insert(pin_id, type_desc);
    }
    
    /// 推断连接的类型
    pub fn infer_connection(&mut self, from_pin: PinId, to_pin: PinId) -> Result<(), String> {
        let from_type = self.get_pin_type(from_pin)?;
        let to_type = self.get_pin_type(to_pin)?;
        
        match (&from_type.data_type, &to_type.data_type) {
            // 具体类型 -> 未知类型：推断为具体类型
            (DataType::Concrete(vtype), DataType::Unknown) => {
                self.set_pin_type(to_pin, DataType::Concrete(vtype.clone()))?;
            }
            
            // 具体类型 -> 类型变量：绑定类型变量
            (DataType::Concrete(vtype), DataType::TypeVar(var_id)) => {
                self.bind_type_var(*var_id, vtype.clone())?;
            }
            
            // 类型变量 -> 类型变量：统一类型变量
            (DataType::TypeVar(var1), DataType::TypeVar(var2)) => {
                self.unify_type_vars(*var1, *var2)?;
            }
            
            // 具体类型 -> 具体类型：检查兼容性
            (DataType::Concrete(from_vtype), DataType::Concrete(to_vtype)) => {
                if !self.is_compatible(from_vtype, to_vtype) {
                    return Err(format!(
                        "Type mismatch: cannot connect {} to {}",
                        from_vtype.to_string(),
                        to_vtype.to_string()
                    ));
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    /// 绑定类型变量
    fn bind_type_var(&mut self, var_id: TypeVarId, vtype: ValueType) -> Result<(), String> {
        // 检查约束
        if let Some(pin_type) = self.find_pin_with_type_var(var_id) {
            for constraint in &pin_type.constraints {
                if !constraint.is_satisfied_by(&vtype) {
                    return Err(format!(
                        "Type {} does not satisfy constraint {:?}",
                        vtype.to_string(),
                        constraint
                    ));
                }
            }
        }
        
        self.type_var_bindings.insert(var_id, vtype);
        Ok(())
    }
    
    /// 统一两个类型变量
    fn unify_type_vars(&mut self, var1: TypeVarId, var2: TypeVarId) -> Result<(), String> {
        // 如果其中一个已经绑定，将另一个也绑定到相同类型
        if let Some(vtype) = self.type_var_bindings.get(&var1).cloned() {
            self.bind_type_var(var2, vtype)?;
        } else if let Some(vtype) = self.type_var_bindings.get(&var2).cloned() {
            self.bind_type_var(var1, vtype)?;
        }
        // 否则，记录它们应该统一（实现时可以用 Union-Find）
        Ok(())
    }
    
    /// 解析 Pin 的最终类型
    pub fn resolve_pin_type(&self, pin_id: PinId) -> Result<ValueType, String> {
        let pin_type = self.get_pin_type(pin_id)?;
        
        match &pin_type.data_type {
            DataType::Concrete(vtype) => Ok(vtype.clone()),
            DataType::TypeVar(var_id) => {
                self.type_var_bindings
                    .get(var_id)
                    .cloned()
                    .ok_or_else(|| format!("Type variable {:?} not bound", var_id))
            }
            DataType::Unknown => Err("Type still unknown".to_string()),
            DataType::Union(_) => Err("Union types not yet supported".to_string()),
        }
    }
    
    /// 检查类型兼容性
    fn is_compatible(&self, from_type: &ValueType, to_type: &ValueType) -> bool {
        // Any 类型兼容所有类型
        if matches!(to_type, ValueType::Any) {
            return true;
        }
        
        // 相同类型兼容
        if from_type == to_type {
            return true;
        }
        
        // 数字类型之间可以转换
        if matches!(from_type, ValueType::Int64 | ValueType::Float64)
            && matches!(to_type, ValueType::Int64 | ValueType::Float64)
        {
            return true;
        }
        
        false
    }
    
    // 辅助方法
    fn get_pin_type(&self, pin_id: PinId) -> Result<&PinTypeDesc, String> {
        self.pin_types
            .get(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not registered", pin_id))
    }
    
    fn set_pin_type(&mut self, pin_id: PinId, data_type: DataType) -> Result<(), String> {
        if let Some(pin_type) = self.pin_types.get_mut(&pin_id) {
            pin_type.data_type = data_type;
            Ok(())
        } else {
            Err(format!("Pin {:?} not registered", pin_id))
        }
    }
    
    fn find_pin_with_type_var(&self, var_id: TypeVarId) -> Option<&PinTypeDesc> {
        self.pin_types
            .values()
            .find(|pt| matches!(&pt.data_type, DataType::TypeVar(v) if *v == var_id))
    }
}
```

## 使用示例

### 示例 1: Print 节点（接受任意类型）

```rust
// Print 节点定义
let print_node = GenericNode::new_prototype("print", "Print");

// Value input：Unknown 类型，等待推断
let value_input = GenericInDataPin::new_with_type_desc(
    uuid::Uuid::nil(),
    "Value",
    PinTypeDesc::unknown()  // 🔑 关键：Unknown 类型
);

print_node.add_in_data_pin(value_input);
```

**连接后的推断**：
```rust
// 连接：Divide.Result (Float64) -> Print.Value (Unknown)
type_inference.infer_connection(divide_result_pin, print_value_pin)?;

// 推断结果：Print.Value 变成 Float64
assert_eq!(
    type_inference.resolve_pin_type(print_value_pin)?,
    ValueType::Float64
);
```

### 示例 2: Add 节点（类型变量）

```rust
// Add 节点定义
let add_node = GenericNode::new_prototype("add", "Add");

let type_var = TypeVarId::new();  // 创建类型变量 T1

// A, B, Result 共享同一个类型变量
let a_input = GenericInDataPin::new_with_type_desc(
    uuid::Uuid::nil(),
    "A",
    PinTypeDesc::type_var_with_constraints(
        type_var,
        vec![TypeConstraint::Numeric]  // 约束：必须是数字
    )
);

let b_input = GenericInDataPin::new_with_type_desc(
    uuid::Uuid::nil(),
    "B",
    PinTypeDesc::type_var(type_var)
);

let result_output = GenericOutDataPin::new_with_type_desc(
    uuid::Uuid::nil(),
    "Result",
    PinTypeDesc::type_var(type_var)
);

add_node.add_in_data_pin(a_input);
add_node.add_in_data_pin(b_input);
add_node.add_output(result_output);
```

**连接后的推断**：
```rust
// 连接：Constant(10.0) -> Add.A
type_inference.infer_connection(constant_output, add_a_input)?;

// 推断结果：T1 = Float64
// Add.A, Add.B, Add.Result 都变成 Float64
assert_eq!(type_inference.resolve_pin_type(add_a_input)?, ValueType::Float64);
assert_eq!(type_inference.resolve_pin_type(add_b_input)?, ValueType::Float64);
assert_eq!(type_inference.resolve_pin_type(add_result_output)?, ValueType::Float64);
```

### 示例 3: Compare 节点（约束）

```rust
// Compare 节点定义
let compare_node = GenericNode::new_prototype("compare", "Compare");

let type_var = TypeVarId::new();

// A, B 必须是可比较类型
let a_input = GenericInDataPin::new_with_type_desc(
    uuid::Uuid::nil(),
    "A",
    PinTypeDesc::type_var_with_constraints(
        type_var,
        vec![TypeConstraint::Comparable]  // 约束：可比较
    )
);

let b_input = GenericInDataPin::new_with_type_desc(
    uuid::Uuid::nil(),
    "B",
    PinTypeDesc::type_var(type_var)
);

// Result 总是 Boolean
let result_output = GenericOutDataPin::new_with_type_desc(
    uuid::Uuid::nil(),
    "Result",
    PinTypeDesc::concrete(ValueType::Boolean)
);
```

## 实现路线图

### Phase 1: 基础结构（立即实现）

1. ✅ 创建 `type_desc.rs` - DataType 枚举
2. ✅ 创建 `type_var.rs` - TypeVarId
3. ✅ 创建 `type_constraint.rs` - TypeConstraint
4. ✅ 创建 `pin_type.rs` - PinTypeDesc

### Phase 2: 推断引擎（核心）

1. ✅ 创建 `type_inference.rs` - TypeInferenceContext
2. ✅ 实现基本推断规则
3. ✅ 实现约束检查

### Phase 3: 集成到现有系统

1. ⏳ 修改 `GenericInDataPin` 和 `GenericOutDataPin` 支持 `PinTypeDesc`
2. ⏳ 在 `ConnectionManager` 中集成类型推断
3. ⏳ 更新节点定义使用新的类型系统

### Phase 4: 前端支持

1. ⏳ 前端显示推断后的类型
2. ⏳ 连接时实时类型检查
3. ⏳ 类型错误提示

## 向后兼容

### 兼容策略

```rust
impl PinTypeDesc {
    /// 从旧的 ValueType 创建（向后兼容）
    pub fn from_value_type(vtype: ValueType) -> Self {
        Self::concrete(vtype)
    }
    
    /// 转换为旧的 ValueType（向后兼容）
    pub fn to_value_type(&self) -> ValueType {
        match &self.data_type {
            DataType::Concrete(vtype) => vtype.clone(),
            DataType::Unknown => ValueType::Any,
            DataType::TypeVar(_) => ValueType::Any,
            DataType::Union(_) => ValueType::Any,
        }
    }
}
```

### 渐进式迁移

1. **阶段 1**：新节点使用 `PinTypeDesc`，旧节点继续使用 `ValueType`
2. **阶段 2**：逐步迁移现有节点
3. **阶段 3**：完全切换到新系统

## 总结

### 优势

1. **灵活性**：支持动态类型推断
2. **类型安全**：编译时和连接时类型检查
3. **可扩展**：约束系统易于扩展
4. **向后兼容**：不破坏现有代码

### 关键特性

- ✅ Unknown 类型（Print 的 input）
- ✅ TypeVar 类型（Add 的 A, B, Result）
- ✅ 类型约束（Numeric, Comparable）
- ✅ 类型推断引擎
- ✅ 向后兼容

### 适用场景

- ✅ Print 节点接受任意类型
- ✅ Add 节点的类型统一
- ✅ Compare 节点的类型约束
- ✅ Filter 节点的泛型支持
- ✅ BI 系统的复杂类型推断
