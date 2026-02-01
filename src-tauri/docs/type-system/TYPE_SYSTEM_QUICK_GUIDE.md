# 类型系统快速指南

## 🎯 快速参考

### 创建 Pin 的三种方式

#### 1. Unknown 类型（接受任意类型）
```rust
use crate::executor::value::PinTypeDesc;
use crate::executor::pin::GenericInDataPin;

// Print 节点的 Value pin
GenericInDataPin::new(
    node_id,
    "Value",
    PinTypeDesc::unknown()
)
```

**使用场景**:
- Print 节点
- 调试节点
- 任何需要接受任意类型的 Pin

#### 2. Concrete 类型（具体类型）
```rust
use crate::executor::value::{PinTypeDesc, ValueType};
use crate::executor::pin::GenericInDataPin;

// Float64 输入
GenericInDataPin::new(
    node_id,
    "Input",
    PinTypeDesc::concrete(ValueType::Float64)
)

// String 输入
GenericInDataPin::new(
    node_id,
    "Text",
    PinTypeDesc::concrete(ValueType::String)
)

// DataFrame 输入
GenericInDataPin::new(
    node_id,
    "Data",
    PinTypeDesc::concrete(ValueType::DataFrame)
)
```

**使用场景**:
- 大多数数据处理节点
- 需要明确类型的 Pin
- 类型转换节点

#### 3. TypeVar 类型（泛型 + 约束）
```rust
use crate::executor::value::{PinTypeDesc, TypeVarId, TypeConstraint};
use crate::executor::pin::GenericInDataPin;

// 创建共享的类型变量
let type_var = TypeVarId::new();

// 数学运算节点（Numeric 约束）
GenericInDataPin::new(
    node_id,
    "A",
    PinTypeDesc::type_var_with_constraints(
        type_var,
        vec![TypeConstraint::Numeric]
    )
)

GenericInDataPin::new(
    node_id,
    "B",
    PinTypeDesc::type_var_with_constraints(
        type_var,  // 共享同一类型变量
        vec![TypeConstraint::Numeric]
    )
)
```

**使用场景**:
- Add, Subtract, Multiply, Divide 等数学运算
- 比较运算符（Comparable 约束）
- 任何需要多个 Pin 类型一致的节点

### 完整的节点示例

#### Example 1: Print 节点（Unknown）
```rust
use crate::executor::value::PinTypeDesc;
use crate::executor::pin::{GenericInDataPin, GenericInExecPin, GenericOutExecPin};
use crate::executor::GenericNode;

pub fn create_print_node(node_id: Uuid) -> GenericNode {
    GenericNode::new(
        node_id,
        "print".to_string(),
        "Print".to_string(),
        vec![
            Box::new(GenericInExecPin::new(node_id, "In")),
        ],
        vec![
            Box::new(GenericInDataPin::new(
                node_id,
                "Value",
                PinTypeDesc::unknown()  // 接受任意类型
            )),
        ],
        vec![
            Box::new(GenericOutExecPin::new(node_id, "Out")),
        ],
        vec![],
        ExecutionModel::Hybrid,
        Box::new(|ctx, node_id| {
            // 实现...
            Ok(())
        }),
    )
}
```

#### Example 2: Add 节点（TypeVar + Numeric）
```rust
use crate::executor::value::{PinTypeDesc, ValueType, TypeVarId, TypeConstraint};
use crate::executor::pin::{GenericInDataPin, GenericOutDataPin};
use crate::executor::GenericNode;

pub fn create_add_node(node_id: Uuid) -> GenericNode {
    let type_var = TypeVarId::new();  // 创建共享类型变量
    
    GenericNode::new(
        node_id,
        "add".to_string(),
        "Add".to_string(),
        vec![],
        vec![
            Box::new(GenericInDataPin::new(
                node_id,
                "A",
                PinTypeDesc::type_var_with_constraints(
                    type_var,
                    vec![TypeConstraint::Numeric]
                )
            )),
            Box::new(GenericInDataPin::new(
                node_id,
                "B",
                PinTypeDesc::type_var_with_constraints(
                    type_var,  // 共享同一类型变量
                    vec![TypeConstraint::Numeric]
                )
            )),
        ],
        vec![],
        vec![
            Box::new(GenericOutDataPin::new(
                node_id,
                "Result",
                PinTypeDesc::type_var_with_constraints(
                    type_var,  // 输出也共享同一类型变量
                    vec![TypeConstraint::Numeric]
                )
            )),
        ],
        ExecutionModel::DataFlow,
        Box::new(|ctx, node_id| {
            // 实现...
            Ok(())
        }),
    )
}
```

#### Example 3: ToFloat 节点（Concrete）
```rust
use crate::executor::value::{PinTypeDesc, ValueType};
use crate::executor::pin::{GenericInDataPin, GenericOutDataPin};
use crate::executor::GenericNode;

pub fn create_to_float_node(node_id: Uuid) -> GenericNode {
    GenericNode::new(
        node_id,
        "to_float".to_string(),
        "To Float".to_string(),
        vec![],
        vec![
            Box::new(GenericInDataPin::new(
                node_id,
                "Input",
                PinTypeDesc::unknown()  // 接受任意类型
            )),
        ],
        vec![],
        vec![
            Box::new(GenericOutDataPin::new(
                node_id,
                "Output",
                PinTypeDesc::concrete(ValueType::Float64)  // 输出固定为 Float64
            )),
        ],
        ExecutionModel::DataFlow,
        Box::new(|ctx, node_id| {
            // 实现...
            Ok(())
        }),
    )
}
```

## 📋 类型约束参考

### TypeConstraint 枚举

```rust
pub enum TypeConstraint {
    /// 数值类型（Float64, Int64）
    Numeric,
    
    /// 可比较类型（支持 <, >, ==）
    Comparable,
    
    /// 可迭代类型（List, DataFrame, Series）
    Iterable,
    
    /// 可序列化类型
    Serializable,
    
    /// 必须是特定类型之一
    OneOf(Vec<ValueType>),
    
    /// 自定义约束
    Custom(String),
}
```

### 使用示例

#### Numeric 约束
```rust
// 数学运算：Add, Subtract, Multiply, Divide
PinTypeDesc::type_var_with_constraints(
    type_var,
    vec![TypeConstraint::Numeric]
)
```

#### Comparable 约束
```rust
// 比较运算：Greater, Less, Equal
PinTypeDesc::type_var_with_constraints(
    type_var,
    vec![TypeConstraint::Comparable]
)
```

#### Iterable 约束
```rust
// 数组操作：Map, Filter, Reduce
PinTypeDesc::type_var_with_constraints(
    type_var,
    vec![TypeConstraint::Iterable]
)
```

#### OneOf 约束
```rust
// 只接受特定类型
PinTypeDesc::type_var_with_constraints(
    type_var,
    vec![TypeConstraint::OneOf(vec![
        ValueType::Float64,
        ValueType::Int64,
    ])]
)
```

#### 多个约束
```rust
// 同时满足多个约束
PinTypeDesc::type_var_with_constraints(
    type_var,
    vec![
        TypeConstraint::Numeric,
        TypeConstraint::Comparable,
    ]
)
```

## 🔄 类型推断流程

### 1. 注册 Pin
```rust
let mut type_inference = TypeInferenceContext::new();

type_inference.register_pin(
    pin_id,
    PinTypeDesc::type_var_with_constraints(
        type_var,
        vec![TypeConstraint::Numeric]
    )
);
```

### 2. 推断连接
```rust
// 尝试连接两个 Pin
match type_inference.infer_connection(output_pin_id, input_pin_id) {
    Ok(_) => {
        // 连接成功
    }
    Err(e) => {
        // 连接失败，显示错误
        println!("Cannot connect: {}", e);
    }
}
```

### 3. 解析类型
```rust
// 获取 Pin 的最终类型
match type_inference.resolve_pin_type(pin_id) {
    Some(value_type) => {
        println!("Pin type: {:?}", value_type);
    }
    None => {
        println!("Pin type not yet resolved");
    }
}
```

## 🎨 前端类型字符串映射

### PinTypeDesc::from_string() 映射表

| 前端字符串 | PinTypeDesc | ValueType |
|-----------|-------------|-----------|
| "any" | Unknown | - |
| "object" | Unknown | - |
| "unknown" | Unknown | - |
| "float64" | Concrete | Float64 |
| "float" | Concrete | Float64 |
| "int64" | Concrete | Int64 |
| "int" | Concrete | Int64 |
| "string" | Concrete | String |
| "bool" | Concrete | Boolean |
| "date" | Concrete | Date |
| "datetime" | Concrete | Datetime |
| "duration" | Concrete | Duration |
| "dataframe" | Concrete | DataFrame |
| "series" | Concrete | Series |
| "array" | Concrete | List(Any) |
| "exec" | Concrete | Any |
| 其他 | Unknown | - |

### 使用示例

```rust
// 前端传来的类型字符串
let frontend_type = "float64";

// 转换为 PinTypeDesc
let pin_type = PinTypeDesc::from_string(frontend_type);

// 注册到类型推断上下文
type_inference.register_pin(pin_id, pin_type);
```

## ⚠️ 常见错误

### 错误 1: 直接使用 ValueType
```rust
// ❌ 错误
GenericInDataPin::new(node_id, "Input", ValueType::Float64)

// ✅ 正确
GenericInDataPin::new(
    node_id,
    "Input",
    PinTypeDesc::concrete(ValueType::Float64)
)
```

### 错误 2: 导入路径错误
```rust
// ❌ 错误
use yssbi_lib::executor::{value::PinTypeDesc, ValueType};

// ✅ 正确
use yssbi_lib::executor::value::{PinTypeDesc, ValueType};
```

### 错误 3: 不共享 TypeVar
```rust
// ❌ 错误：每个 Pin 都有不同的 TypeVar
let type_var_a = TypeVarId::new();
let type_var_b = TypeVarId::new();

GenericInDataPin::new(node_id, "A", PinTypeDesc::type_var(type_var_a))
GenericInDataPin::new(node_id, "B", PinTypeDesc::type_var(type_var_b))

// ✅ 正确：共享同一 TypeVar
let type_var = TypeVarId::new();

GenericInDataPin::new(node_id, "A", PinTypeDesc::type_var(type_var))
GenericInDataPin::new(node_id, "B", PinTypeDesc::type_var(type_var))
```

### 错误 4: 序列化字段名
```rust
// ❌ 错误
assert_eq!(pin_json["type"], "float64");

// ✅ 正确
assert_eq!(pin_json["pin_type"], "float64");
```

## 📚 相关文档

- `TYPE_INFERENCE_DESIGN.md` - 类型推断系统设计
- `TYPE_INFERENCE_IMPLEMENTATION.md` - 实现细节
- `TYPE_INFERENCE_REFACTOR_STATUS.md` - 重构状态
- `PROJECT_STATUS_SUMMARY.md` - 项目状态总结

## 🚀 快速开始

### 创建新节点的步骤

1. **确定节点类型**
   - 需要接受任意类型？→ 使用 `Unknown`
   - 需要具体类型？→ 使用 `Concrete`
   - 需要泛型？→ 使用 `TypeVar`

2. **创建 Pin**
   ```rust
   use crate::executor::value::{PinTypeDesc, ValueType};
   use crate::executor::pin::GenericInDataPin;
   
   // 根据需求选择合适的类型
   let pin = GenericInDataPin::new(
       node_id,
       "Input",
       PinTypeDesc::concrete(ValueType::Float64)
   );
   ```

3. **创建节点**
   ```rust
   use crate::executor::GenericNode;
   
   let node = GenericNode::new(
       node_id,
       "my_node".to_string(),
       "My Node".to_string(),
       vec![],  // exec_inputs
       vec![Box::new(pin)],  // data_inputs
       vec![],  // exec_outputs
       vec![],  // data_outputs
       ExecutionModel::DataFlow,
       Box::new(|ctx, node_id| {
           // 实现节点逻辑
           Ok(())
       }),
   );
   ```

4. **注册节点**
   ```rust
   use crate::executor::node::registry::get_registry;
   
   let registry = get_registry();
   registry.register(node);
   ```

完成！你的节点现在支持完整的类型推断系统。
