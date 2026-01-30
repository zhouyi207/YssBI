# 连接线逻辑与类型推断分析

## 📋 当前连接流程

### 1. 前端发起连接请求

用户在 UI 中拖拽连接线，前端调用 Tauri 命令：

```typescript
// 前端调用
await invoke('connect_pins', {
  subgraphId: 'event-xxx',
  sourcePinId: 'pin-xxx',
  targetPinId: 'pin-yyy'
});
```

### 2. 后端处理连接 (state/node_crud.rs)

```rust
pub fn connect_pins(
    &self,
    subgraph_id: &str,
    source_pin_id: &str,
    target_pin_id: &str,
) -> Result<Vec<SerializedNode>, String>
```

**处理步骤：**

1. **查找 Pin 信息**
   - 遍历所有节点，找到 source 和 target pin
   - 获取 pin 的类型信息 (pin.pin_type)

2. **验证方向**
   - 确保一个是输出 pin，一个是输入 pin
   - 不允许同方向的 pin 连接

3. **类型兼容性检查** ⚠️ **关键点**
   ```rust
   if !can_connect(output_type, input_type) {
       return Err("type not compatible");
   }
   ```
   - 使用 `schema/pin_types.rs` 中的 `can_connect()` 函数
   - 基于字符串类型名称进行检查（如 "float64", "string", "any"）
   - **问题：这里没有使用类型推断系统！**

4. **移除旧连接**
   - 输入 pin 只能有一个连接（单连接）
   - 移除输入 pin 的旧连接

5. **建立新连接**
   - 更新输出 pin 的 links 数组
   - 更新输入 pin 的 links 数组

### 3. 执行时的连接处理 (executor/connection.rs)

在执行图时，`ConnectionManager` 负责管理运行时连接：

```rust
pub fn connect(
    &self,
    from_pin: &Arc<dyn OutDataPin>,
    to_pin: &Arc<dyn InDataPin>,
) -> ConnectionResult<()>
```

**处理步骤：**

1. **类型推断** ✅ **已集成**
   ```rust
   let mut type_inference = self.type_inference.lock().unwrap();
   if let Err(_e) = type_inference.infer_connection(from_id, to_id) {
       // 类型推断失败，回退到旧的类型检查
       if from_pin.data_type() != to_pin.data_type() { ... }
   }
   ```

2. **循环检测**
   - 使用 DFS 检测是否会形成循环

3. **建立连接**
   - 在 connections HashMap 中记录连接关系

## 🔍 问题分析

### 问题 1: 类型检查不一致

**前端连接时 (state/node_crud.rs):**
- 使用 `can_connect()` 函数
- 基于字符串类型名称（"float64", "string", "any"）
- **不支持类型推断**
- **不支持 TypeVar**
- **不支持 Unknown 类型**

**执行时 (executor/connection.rs):**
- 使用 `TypeInferenceContext.infer_connection()`
- 支持完整的类型推断系统
- 支持 TypeVar、Unknown、Concrete、Union
- 支持类型约束（Numeric, Comparable 等）

### 问题 2: 前端类型信息不完整

从你提供的 JSON 数据来看：

```json
{
  "id": "pin-403c62bd-a44e-4a0b-96ba-0b75309c03fc",
  "name": "Value",
  "type": "any",  // ⚠️ 这是字符串类型名称
  "links": [],
  "isArray": false
}
```

**当前问题：**
- `type` 字段只是简单的字符串（"any", "float64", "string"）
- 没有 TypeVar 信息
- 没有类型约束信息
- 没有 Unknown vs Concrete 的区分

## ✅ 解决方案

### 方案 1: 在前端连接时集成类型推断（推荐）

修改 `state/node_crud.rs` 中的 `connect_pins()` 函数：

```rust
pub fn connect_pins(
    &self,
    subgraph_id: &str,
    source_pin_id: &str,
    target_pin_id: &str,
) -> Result<Vec<SerializedNode>, String> {
    // ... 查找 pin 信息 ...
    
    // ✅ 新增：使用类型推断系统进行类型检查
    let type_inference = TypeInferenceContext::new();
    
    // 注册 pin 类型
    type_inference.register_pin(
        source_pin_id, 
        PinTypeDesc::from_string(&source_type)
    );
    type_inference.register_pin(
        target_pin_id, 
        PinTypeDesc::from_string(&target_type)
    );
    
    // 尝试推断连接
    if let Err(e) = type_inference.infer_connection(source_pin_id, target_pin_id) {
        return Err(format!("Type inference failed: {}", e));
    }
    
    // ... 建立连接 ...
}
```

### 方案 2: 扩展前端类型信息

修改前端的 Pin 数据结构，包含完整的类型信息：

```typescript
interface Pin {
  id: string;
  name: string;
  type: string;  // 保留向后兼容
  
  // ✅ 新增：完整的类型描述
  typeDesc?: {
    kind: 'Unknown' | 'Concrete' | 'TypeVar' | 'Union';
    concreteType?: string;  // 如果是 Concrete
    typeVarId?: number;     // 如果是 TypeVar
    constraints?: string[]; // 类型约束
    unionTypes?: string[];  // 如果是 Union
  };
  
  links: string[];
  isArray: boolean;
}
```

### 方案 3: 添加类型推断 API

创建新的 Tauri 命令，供前端查询类型兼容性：

```rust
#[tauri::command]
fn check_pin_compatibility(
    source_pin_id: &str,
    target_pin_id: &str,
    source_type: &str,
    target_type: &str,
) -> Result<bool, String> {
    let mut ctx = TypeInferenceContext::new();
    
    ctx.register_pin(
        source_pin_id.parse().unwrap(),
        PinTypeDesc::from_string(source_type)
    );
    ctx.register_pin(
        target_pin_id.parse().unwrap(),
        PinTypeDesc::from_string(target_type)
    );
    
    match ctx.infer_connection(
        source_pin_id.parse().unwrap(),
        target_pin_id.parse().unwrap()
    ) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
```

前端在拖拽连接线时实时调用：

```typescript
// 用户拖拽连接线时
const canConnect = await invoke('check_pin_compatibility', {
  sourcePinId: 'pin-xxx',
  targetPinId: 'pin-yyy',
  sourceType: 'float64',
  targetType: 'any'
});

if (!canConnect) {
  // 显示错误提示，不允许连接
}
```

## 🎯 推荐实施步骤

### Phase 4.1: 后端集成（立即实施）

1. **修改 `connect_pins()` 函数**
   - 在 `state/node_crud.rs` 中集成类型推断
   - 替换 `can_connect()` 为 `TypeInferenceContext.infer_connection()`

2. **添加类型转换辅助函数**
   ```rust
   impl PinTypeDesc {
       pub fn from_string(type_str: &str) -> Self {
           match type_str {
               "any" => PinTypeDesc::unknown(),
               _ => PinTypeDesc::concrete(ValueType::from_string(type_str))
           }
       }
   }
   ```

3. **保持向后兼容**
   - 如果类型推断失败，回退到旧的 `can_connect()` 检查
   - 逐步迁移，不破坏现有功能

### Phase 4.2: 前端 API（短期）

1. **添加类型兼容性检查 API**
   - 创建 `check_pin_compatibility` Tauri 命令
   - 前端在拖拽时实时检查

2. **添加类型信息查询 API**
   - 创建 `get_pin_type_info` Tauri 命令
   - 返回完整的类型描述（TypeVar, Unknown, Concrete）

### Phase 4.3: 前端 UI（中期）

1. **显示类型信息**
   - 在 Pin 上显示类型（Unknown, TypeVar, Concrete）
   - 显示类型约束（Numeric, Comparable）

2. **实时类型验证**
   - 拖拽连接线时显示是否兼容
   - 不兼容的连接显示红色/禁止图标

3. **类型推断反馈**
   - 连接后显示推断出的类型
   - 高亮显示类型变化

## 📊 当前状态总结

### ✅ 已完成：
- 类型推断引擎实现 (TypeInferenceContext)
- 执行时连接管理集成类型推断
- 所有节点使用 PinTypeDesc API

### ⚠️ 待完成：
- **前端连接时的类型推断集成** ← 最重要
- 类型兼容性检查 API
- 前端类型信息显示
- 实时类型验证 UI

### 🔧 需要修改的文件：
1. `src-tauri/src/state/node_crud.rs` - connect_pins() 函数
2. `src-tauri/src/executor/value/type_desc.rs` - 添加 from_string() 方法
3. `src-tauri/src/lib.rs` - 添加新的 Tauri 命令
4. 前端 TypeScript 文件 - 调用新 API

## 🚀 立即行动项

**最紧急：修复前端连接时的类型检查**

当前你的 JSON 数据显示 Print 节点的 Value pin 类型是 "any"，这应该被识别为 `Unknown` 类型。但是 `connect_pins()` 函数使用的是旧的字符串比较逻辑。

**建议：**
1. 先实施 Phase 4.1（后端集成）
2. 确保前端连接时也使用类型推断系统
3. 然后再考虑 UI 改进

这样可以确保类型推断系统在整个流程中保持一致。
