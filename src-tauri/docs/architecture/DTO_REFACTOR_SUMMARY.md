# DTO 重构总结

## 重构日期
2026-02-02

## 重构目标
1. 统一命名：将 `*Data` 后缀改为 `*Dto`，明确表示数据传输对象
2. 优化组织：将 DTO 从 `executor` 模块移到 `project` 模块，职责更清晰

## 命名变更

| 旧名称 | 新名称 | 说明 |
|--------|--------|------|
| `NodeData` | `NodeDto` | 节点数据传输对象 |
| `PinData` | `PinDto` | Pin 数据传输对象 |
| `GraphData` | `GraphDto` | 图数据传输对象 |
| `VariableData` | `VariableDto` | 变量数据传输对象 |
| `PinDefinition` | `PinDefDto` | Pin 定义数据传输对象 |

## 文件结构变更

### 删除
- `src/executor/node/data.rs` ❌

### 新增
- `src/project/dto.rs` ✅

### 修改
- `src/project/mod.rs` - 导出 DTO 模块
- `src/executor/node/mod.rs` - 移除 data 模块声明
- `src/executor/mod.rs` - 从 project 重新导出 DTO
- `src/executor/node/implementation.rs` - 更新导入路径
- `src/executor/processors.rs` - 更新导入路径
- `src/executor/context.rs` - 更新导入路径

## 模块职责划分

### `project` 模块
**职责**：数据序列化、持久化、前端交互

包含：
- `dto.rs` - 数据传输对象（NodeDto, PinDto, GraphDto, VariableDto）
- `mod.rs` - 项目数据结构（ProjectData, SubGraphData, SerializedNode）
- `io.rs` - 文件读写操作

### `executor` 模块
**职责**：图执行逻辑、运行时管理

包含：
- `node/implementation.rs` - 运行时节点（GenericNode）
- `context.rs` - 执行上下文
- `processors.rs` - 节点处理器

## DTO vs 其他数据结构

### NodeDto vs SerializedNode
- **NodeDto**：用于执行时的图表示，不包含位置信息
- **SerializedNode**：用于项目文件持久化，包含 position、canvas 等 UI 信息

### NodeDto vs GenericNode
- **NodeDto**：贫血模型，只包含数据，用于序列化
- **GenericNode**：充血模型，包含执行逻辑、处理器、运行时状态

## 依赖关系

```
前端 ←→ DTO ←→ 运行时
     (JSON)   (转换)

project/dto.rs (DTO)
    ↓ 被引用
executor/context.rs (执行器)
executor/node/implementation.rs (运行时节点)
```

## 影响范围

### 自动替换（Python 脚本）
- 18 个文件
- 202 处命名替换

### 手动修改
- 文件移动和模块重组
- 导入路径更新
- 模块声明调整

## 验证结果

✅ `cargo check` 通过
✅ 所有编译警告仅为静态变量引用（与重构无关）
✅ 模块依赖关系清晰

## 后续建议

1. **考虑进一步合并**：`PinDto` 和 `SerializedPin` 非常相似，可以考虑统一
2. **类型别名**：如果需要向后兼容，可以添加类型别名：
   ```rust
   #[deprecated(note = "Use NodeDto instead")]
   pub type NodeData = NodeDto;
   ```
3. **文档更新**：更新相关文档中的类型引用

## 收益

1. **命名清晰**：`Dto` 后缀明确表示数据传输对象，符合业界标准
2. **职责分离**：`project` 负责数据，`executor` 负责执行
3. **依赖正确**：executor 依赖 project 的 DTO，而不是反向依赖
4. **易于维护**：相关数据结构集中在一个模块中
