# 节点类型修复计划

## 问题描述
当前很多节点使用了模糊类型（`any`、`unknown`），导致类型不明确。应该只有运算符节点（`+`、`-`、`*`、`/`）需要类型推断，其他节点应该使用确定的类型。

## 类型分类

### 1. 应该使用确定类型的节点

#### Debug 节点
- **Print**: 
  - 当前：`PinTypeDesc::unknown()` ❌
  - 应该：`PinTypeDesc::concrete(ValueType::String)` ✅
  - 原因：打印应该接受字符串，其他类型需要先转换

#### Control 节点
- **Branch**:
  - 当前：`TrueValue`, `FalseValue`, `Result` 都是 `any()` ❌
  - 应该：保持 `any()` ✅（因为可以传递任意类型）
  - 说明：这个节点类似三元运算符，需要支持任意类型

- **For Each**:
  - 当前：`Array` 是 `any().array()`, `Item` 是 `any()` ❌
  - 应该：保持 `any()` ✅（因为可以遍历任意类型数组）
  - 说明：这是泛型节点，需要支持任意类型

#### Data 节点
- **Array Info**:
  - 当前：`Array` 是 `List(Any)`, `First/Last` 是 `Any` ❌
  - 应该：保持 `Any` ✅（因为数组可以包含任意类型）
  
- **Partition Array**:
  - 当前：`Array`, `Before`, `After`, `AtIndex` 都是 `Any` ❌
  - 应该：保持 `Any` ✅（因为数组可以包含任意类型）

- **Filter Array Multi**:
  - 当前：`Array`, `Above`, `Below`, `Equal` 都是 `Any` ❌
  - 应该：保持 `Any` ✅（因为数组可以包含任意类型）

#### Math 节点
- **Min Max Average**:
  - 当前：`Array` 是 `List(Any)` ❌
  - 应该：`List(Float64)` ✅（因为只能计算数字）

- **Statistics**:
  - 当前：`Array` 是 `List(Any)` ❌
  - 应该：`List(Float64)` ✅（因为只能计算数字）

#### String 节点
- **Split String**:
  - 当前：`Array` 是 `List(Any)` ❌
  - 应该：`List(String)` ✅（因为分割字符串返回字符串数组）

### 2. 应该保持模糊类型的节点（需要类型推断）

#### Math 运算符
- `+`, `-`, `*`, `/`, `%`, `^`
- 原因：支持多种数值类型（Int, Float）

#### 比较运算符
- `==`, `!=`, `>`, `<`, `>=`, `<=`
- 原因：可以比较多种类型

#### 逻辑运算符
- `&&`, `||`, `!`
- 原因：操作布尔值，但输入可能需要转换

#### 泛型容器操作
- **Branch**: 条件分支，传递任意类型
- **For Each**: 遍历任意类型数组
- **Array Info**: 获取任意类型数组信息
- **Partition Array**: 分割任意类型数组
- **Filter Array**: 过滤任意类型数组

## 修复方案

### 修复 1: Print 节点
**文件**: `src-tauri/src/executor/node/catalog/debug.rs`

**当前**:
```rust
print_node.add_in_data_pin(GenericInDataPin::new(
    uuid::Uuid::nil(),
    "Value",
    PinTypeDesc::unknown()  // ❌ 模糊类型
));
```

**修复后**:
```rust
print_node.add_in_data_pin(GenericInDataPin::new(
    uuid::Uuid::nil(),
    "Value",
    PinTypeDesc::concrete(ValueType::String)  // ✅ 确定类型
));
```

**说明**: Print 应该接受字符串。如果用户想打印其他类型，需要先转换为字符串。

### 修复 2: Min Max Average 节点
**文件**: `src-tauri/src/executor/node/catalog/math/multi_output.rs`

**当前**:
```rust
node.add_in_data_pin(GenericInDataPin::new(
    uuid::Uuid::nil(), 
    "Array", 
    PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))  // ❌
));
```

**修复后**:
```rust
node.add_in_data_pin(GenericInDataPin::new(
    uuid::Uuid::nil(), 
    "Array", 
    PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Float64)))  // ✅
));
```

### 修复 3: Statistics 节点
**文件**: `src-tauri/src/executor/node/catalog/math/multi_output.rs`

**当前**:
```rust
node.add_in_data_pin(GenericInDataPin::new(
    uuid::Uuid::nil(), 
    "Array", 
    PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))  // ❌
));
```

**修复后**:
```rust
node.add_in_data_pin(GenericInDataPin::new(
    uuid::Uuid::nil(), 
    "Array", 
    PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Float64)))  // ✅
));
```

### 修复 4: Split String 节点
**文件**: `src-tauri/src/executor/node/catalog/string_multi_output.rs`

**当前**:
```rust
node.add_out_data_pin(GenericOutDataPin::new(
    uuid::Uuid::nil(), 
    "Array", 
    PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::Any)))  // ❌
));
```

**修复后**:
```rust
node.add_out_data_pin(GenericOutDataPin::new(
    uuid::Uuid::nil(), 
    "Array", 
    PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::String)))  // ✅
));
```

### 保持不变的节点

以下节点应该保持 `any()` 类型，因为它们是泛型操作：

1. **Branch** - 条件分支，可以传递任意类型
2. **For Each** - 遍历任意类型数组
3. **Array Info** - 获取任意类型数组信息（First, Last）
4. **Partition Array** - 分割任意类型数组
5. **Filter Array Multi** - 过滤任意类型数组
6. **Get Object Props** - 对象属性可以是任意类型

## 类型系统设计原则

### 1. 确定类型优先
- 如果节点的输入/输出类型是明确的，使用 `concrete(ValueType::XXX)`
- 例如：Print 接受 String，Math 节点接受 Float64

### 2. 泛型类型例外
- 容器操作（数组、对象）可以使用 `Any`
- 条件分支、循环等控制流可以使用 `Any`
- 原因：这些操作不关心具体类型，只是传递数据

### 3. 类型推断节点
- 运算符节点使用类型推断系统
- 可以根据输入自动推断输出类型
- 例如：`Int + Int = Int`, `Float + Float = Float`

### 4. 类型转换节点
- 提供显式类型转换节点
- 例如：`ToString`, `ToInt`, `ToFloat`
- 用户可以在需要时手动转换类型

## 实施步骤

1. ✅ 修复 Print 节点 → String
2. ✅ 修复 Min Max Average → Float64 数组
3. ✅ 修复 Statistics → Float64 数组
4. ✅ 修复 Split String → String 数组
5. ⚠️ 保持 Branch, For Each 等泛型节点为 Any
6. ⚠️ 保持运算符节点使用类型推断

## 影响评估

### 破坏性变更
- **Print 节点**: 之前可以接受任意类型，现在只接受 String
  - 影响：用户需要先转换为字符串
  - 解决：提供 ToString 转换节点

- **Math 节点**: 之前可以接受任意类型数组，现在只接受 Float64 数组
  - 影响：用户需要确保数组元素是数字
  - 解决：提供类型检查和转换

### 优势
1. **类型安全**: 编译时发现类型错误
2. **更好的提示**: IDE 可以提供更准确的类型提示
3. **减少运行时错误**: 类型不匹配在连接时就能发现
4. **更清晰的 API**: 用户知道每个节点期望什么类型

## 后续工作

1. **添加类型转换节点**:
   - ToString
   - ToInt
   - ToFloat
   - ToBool

2. **改进类型推断系统**:
   - 更智能的类型推断
   - 自动插入类型转换

3. **类型检查**:
   - 连接时检查类型兼容性
   - 提供友好的错误提示

4. **文档更新**:
   - 更新节点文档，说明类型要求
   - 提供类型转换示例

## 状态
📝 **计划阶段 - 等待确认**

需要确认：
1. Print 节点是否应该只接受 String？
2. 是否需要添加类型转换节点？
3. 是否需要保持向后兼容？
