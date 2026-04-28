# 前后端 DTO 类型映射文档

本文档对照后端原始类型、DTO 类型、前端转换后的类型，说明用于 IPC 传输的数据结构及字段含义。

---

## 一、类型转换流程概览

```
后端原始类型 (Rust)     →    DTO 类型 (Schema)     →    前端接收/转换后类型 (TypeScript)
─────────────────────────────────────────────────────────────────────────────────────
GraphInstance           →    GraphInstanceDTO     →    Graph (domain) / GraphData (store)
NodeInstance            →    NodeInstanceDTO      →    Node (domain) / NodeData (store)
PinInstance             →    PinInstanceDTO       →    Pin (domain) / PinData (store)
Connection              →    ConnectionItemDTO    →    ConnectionItem / ConnectionData
ProjectData             →    ProjectDataDTO       →    ProjectData (domain)
```

**序列化约定**：所有 DTO 使用 `#[serde(rename_all = "camelCase")]`，JSON 键为 camelCase。`GraphTypeDTO` 使用 `rename_all = "lowercase"`，序列化为 `"event"` / `"function"`。

---

## 二、Graph（图）

### 2.1 类型对照表

| 字段 | 后端原始类型 (GraphInstance) | DTO 类型 (GraphInstanceDTO) | 前端 Domain (Graph) | 前端 Store (GraphData) | 说明 |
|------|------------------------------|-----------------------------|---------------------|------------------------|------|
| id | GraphId (UUID) | id | string | string | 图唯一标识 |
| name | String | name | string | string | 图名称 |
| type | GraphKind (enum) | type (graph_type → rename) | GraphType | 'event'\|'function' | 图类型，DTO 用 `type` 避免 JS 保留字 |
| nodes | HashMap<NodeId, NodeInstance> | NodeInstanceDTO[] | Node[] | NodeData[] | 节点列表，DTO 展平为数组 |
| pins | (在 data_state 中) | PinInstanceDTO[] | Pin[] | PinData[] | 所有 Pin，DTO 展平为数组 |
| connections | ConnectionManager | connections: ConnectionDTO | Connection | ConnectionData[] 或 { connections } | 连接关系 |
| canvas | GraphPosition (position) | canvas | GraphPosition | GraphPosition | 画布视口 (x, y, scale) |

### 2.2 字段说明

- **type**：后端 `graph_type` 字段通过 `#[serde(rename = "type")]` 序列化为 `type`，与前端保持一致。
- **nodes / pins**：后端以 HashMap 存储，DTO 转为数组便于 JSON 序列化；前端 Domain 的 Node 含完整 Pin 对象，Store 的 NodeData 仅存 Pin ID 数组。
- **connections**：后端 ConnectionManager 内部为 HashMap 结构，DTO 转为 `{ connections: ConnectionItemDTO[] }`；Store 的 GraphData 支持 `ConnectionData[]` 或 `{ connections: Array<{ fromPin, toPin }> }` 两种格式。

---

## 三、Node（节点）

### 3.1 类型对照表

| 字段 | 后端原始类型 (NodeInstance) | DTO 类型 (NodeInstanceDTO) | 前端 Domain (Node) | 前端 Store (NodeData) | 说明 |
|------|-----------------------------|----------------------------|--------------------|------------------------|------|
| id | NodeId (UUID) | id | string | string | 节点唯一标识 |
| nodeType | definition.name | nodeType (node_type → rename) | node_type | node_type | 节点类型（如 "Value:Constants:Boolean"） |
| category | definition.category | category | string[] | string[] | 分类路径 |
| title | definition.name | title | string | string | 显示标题 |
| inputs | (从 GraphDataState 推导) | inputs: string[] | Pin[] | string[] | 输入 Pin：DTO 为 ID 数组，Domain 为完整 Pin 对象，Store 为 ID 数组 |
| outputs | (从 GraphDataState 推导) | outputs: string[] | Pin[] | string[] | 输出 Pin，同上 |
| uiStyle | definition.metadata.ui_style | uiStyle (ui_style → camelCase) | ui_style | ui_style | UI 样式名（如 "default", "math"） |
| description | definition.metadata.description | description? | description? | description? | 可选描述 |
| position | NodePosition { x, y } | position | (在 Node 中) | { x, y } | 画布坐标 |
| graphId | - | - | - | string | Store 专用，所属图 ID |

### 3.2 字段说明

- **nodeType**：后端来自 `NodeInstance.definition.name`，即节点定义的完整类型名；DTO 中 `node_type` 通过 `#[serde(rename = "nodeType")]` 序列化。
- **inputs / outputs**：后端按 Pin 的 direction 从 GraphDataState 汇总；DTO 仅传 Pin ID，前端 `convertGraphFromDTO` 会从 pins 数组组装完整 Pin 对象填入 Node.inputs/outputs。
- **position**：后端 `NodePosition` 为 `{ x: f32, y: f32 }`，与前端一致。
- **命名差异**：DTO 使用 camelCase（nodeType, uiStyle），Domain/Store 使用 snake_case（node_type, ui_style）；转换时需注意映射。

---

## 四、Pin（针脚）

### 4.1 类型对照表

| 字段 | 后端原始类型 (PinInstance) | DTO 类型 (PinInstanceDTO) | 前端 Domain (Pin) | 前端 Store (PinData) | 说明 |
|------|----------------------------|---------------------------|-------------------|----------------------|------|
| id | PinId (UUID) | id | string | string | Pin 唯一标识 |
| nodeId | node_id | nodeId (node_id → camelCase) | nodeId | nodeId | 所属节点 ID |
| name | definition.name | name | string | string | 显示名称 |
| type | (从 definition.kind/data_type 推导) | type (pin_type → rename) | PinType | string | "exec" 或数据类型（"int","float" 等） |
| direction | definition.direction | direction | PinDirection | PinDirection | "input" \| "output" |
| links | (在 ConnectionManager 中) | links: PinId[] | string[] | string[] | 连接的目标 Pin ID，DTO 常为空由前端填充 |
| defaultValue | - | default_value? (→ defaultValue) | defaultValue? | defaultValue? | 默认值 |
| userValue | user_value | user_value? (→ userValue) | userValue? | userValue? | 用户覆盖值 |
| isArray | - | is_array? (→ isArray) | isArray? | isArray? | 是否为数组类型 |
| ui | - | ui? (PinUIDTO) | ui? | ui? | UI 配置 (x, y, color) |

### 4.2 字段说明

- **type**：后端 `pin_type` 通过 `#[serde(rename = "type")]` 序列化为 `type`；Exec Pin 为 `"exec"`，Data Pin 从 `data_type` 推导（如 `"int"`, `"float"`）。
- **direction**：后端 enum 序列化为 `"input"` / `"output"`（lowercase）。
- **links**：连接关系由 ConnectionManager 管理，DTO 的 links 常为空；前端 `applyConnectionsToPins` 会根据 connections 填充。

---

## 五、Connection（连接）

### 5.1 类型对照表

| 字段 | 后端原始类型 (Connection) | DTO 类型 (ConnectionItemDTO) | 前端 Domain (ConnectionItem) | 前端 Store (ConnectionData) | 说明 |
|------|---------------------------|------------------------------|------------------------------|----------------------------|------|
| fromPin | from_pin | fromPin (from_pin → camelCase) | fromPin | from | 源 Pin ID（输出） |
| toPin | to_pin | toPin (to_pin → camelCase) | toPin | to | 目标 Pin ID（输入） |
| id | - | - | - | "from->to" | Store 派生 ID，用于索引 |

### 5.2 字段说明

- **fromPin / toPin**：后端 `Connection` 为 `{ from_pin, to_pin }`，DTO 使用 `rename_all = "camelCase"` 序列化为 `fromPin`, `toPin`。
- **ConnectionData**：Store 层增加 `id` 字段，格式为 `"${from}->${to}"`，便于 Map 存储与查询；`connectionItemToConnectionData` 负责 DTO → Store 转换。

---

## 六、Position（位置）

### 6.1 NodePosition（节点位置）

| 字段 | 后端 (NodePosition) | DTO | 前端 | 说明 |
|------|---------------------|-----|------|------|
| x | f32 | x | number | 节点左上角 X |
| y | f32 | y | number | 节点左上角 Y |

### 6.2 GraphPosition（画布视口）

| 字段 | 后端 (GraphPosition) | DTO | 前端 | 说明 |
|------|----------------------|-----|------|------|
| x | f64 | x | number | 画布平移 X |
| y | f64 | y | number | 画布平移 Y |
| scale | f64 | scale | number | 缩放比例 |

---

## 七、Project（项目）

### 7.1 ProjectMetadata

| 字段 | 后端 (ProjectMetadata) | DTO (ProjectMetadataDTO) | 前端 (ProjectMetadata) | 说明 |
|------|------------------------|--------------------------|------------------------|------|
| exportTime | export_time | exportTime (camelCase) | exportTime | 导出时间 (RFC3339) |
| appVersion | app_version | appVersion (camelCase) | appVersion | 应用版本号 |

### 7.2 ProjectData

| 字段 | 后端 (ProjectData) | DTO (ProjectDataDTO) | 前端 (ProjectData) | 说明 |
|------|--------------------|----------------------|-------------------|------|
| variables | HashMap<String, VariableDefinition> | variables | Record<string, Variable> | 变量定义 |
| graphs | HashMap<GraphId, GraphInstance> | graphs | Record<string, GraphInstanceDTO> | 图实例 |
| databases | HashMap<String, DatabaseDecl> | databases | Record<string, DatabaseDeclDTO> | 数据库声明 |
| metadata | ProjectMetadata | metadata | ProjectMetadata | 项目元数据 |

---

## 八、Variable（变量）

### 8.1 VariableDefinitionDTO

| 字段 | 后端 (VariableDefinition) | DTO (VariableDefinitionDTO) | 前端 | 说明 |
|------|----------------------------|-----|------|------|
| id | id | id | id | 变量 ID |
| name | name | name | name | 变量名 |
| dataType | data_type | dataType | dataType | 数据类型枚举 |
| description | description | description | description | 描述 |
| scope | scope | scope | scope | 作用域 (Global/Event/Function) |
| staticValue | static_value? | staticValue? | - | 静态初始值 |
| sourceConfig | source_config? | sourceConfig? | - | 数据来源配置 |
| isArray | is_array | isArray | isArray | 是否数组 |
| isConstant | is_constant | isConstant | isConstant | 是否常量 |
| defaultValue | default_value? | defaultValue? | - | 默认值 |
| isExposed | is_exposed | isExposed | isExposed | 是否暴露 |
| tags | tags | tags | tags | 标签 |

---

## 九、Database（数据库）

### 9.1 DatabaseDeclDTO

| 字段 | 后端 (DatabaseDecl) | DTO | 说明 |
|------|---------------------|-----|------|
| id | id | id | 数据库 ID |
| engine | engine | engine | 引擎类型 (Sql/Csv/Parquet/InMemory) |
| schemaVersion | schema_version | schemaVersion (camelCase) | 模式版本 |
| required | required | required | 是否必需 |

### 9.2 DatabaseEngineDTO（枚举）

- **Sql**：`{ engine: DatabaseEngineSqlDTO, connectionString }`（connection_string → camelCase）
- **Csv**：`{ path, delimiter, hasHeader, inferSchemaLength? }`（has_header, infer_schema_length → camelCase）
- **Parquet**：`{ path, columns? }`
- **InMemory**：`{ name }`

---

## 十、转换函数与数据流

| 场景 | 转换函数/位置 | 说明 |
|------|---------------|------|
| DTO → Domain (单图) | `convertGraphFromDTO` (dtoConverters.ts) | 将 GraphInstanceDTO 转为 Graph，从 pins 组装 Node.inputs/outputs 为完整 Pin 对象 |
| DTO → Store | `connectionItemToConnectionData` (dtoConverters) | ConnectionItemDTO → ConnectionData |
| Store → DTO | `connectionDataToItem` (dtoConverters) | ConnectionData → ConnectionItemDTO |
| 项目加载 | `convertProjectDataFromDTO` → `convertGraphsFromDTO` | ProjectDataDTO.graphs 逐图调用 convertGraphFromDTO |
| Pin links 填充 | `applyConnectionsToPins` (dtoConverters) | 根据 connections 更新 Pin.links |

---

## 十一、注意事项

1. **inputs/outputs 双态**：DTO 和 Store 使用 Pin ID 数组；Domain 的 Node 在渲染时使用完整 Pin 对象，由 `convertGraphFromDTO` 从 pins 数组组装。
2. **Connection 命名**：JSON 统一使用 camelCase `fromPin`/`toPin`；Store 的 ConnectionData 使用 `from`/`to` 并派生 `id`。
3. **Pin direction**：后端序列化为 `"input"`/`"output"`，与前端 PinDirection 一致。
4. **Node 命名**：Domain/Store 使用 snake_case（node_type, ui_style），DTO 使用 camelCase（nodeType, uiStyle）；转换时需注意字段映射。
