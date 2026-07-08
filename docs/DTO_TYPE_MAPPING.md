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
- **inputs / outputs**：后端按 Pin 的 direction 从 GraphDataState 汇总；DTO 和 Store 均只保存 Pin ID，前端渲染层从 Store 的 `pins` / `pinConnections` 派生完整 `PinView`。
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
| defaultValue | - | default_value? (→ defaultValue) | defaultValue? | defaultValue? | 默认值 |
| userValue | user_value | user_value? (→ userValue) | userValue? | userValue? | 用户覆盖值 |
| isArray | - | is_array? (→ isArray) | isArray? | isArray? | 是否为数组类型 |
| ui | - | ui? (PinUIDTO) | ui? | ui? | UI 配置 (x, y, color) |

### 4.2 字段说明

- **type**：后端 `pin_type` 通过 `#[serde(rename = "type")]` 序列化为 `type`；Exec Pin 为 `"exec"`，Data Pin 从 `data_type` 推导（如 `"int"`, `"float"`）。
- **direction**：后端 enum 序列化为 `"input"` / `"output"`（lowercase）。
- **连接状态**：连接关系由 ConnectionManager / `connections` 表达；前端 Store 使用 `pinConnections` 作为索引，渲染时派生 `connected` / `linkCount` / `connectionIds`，不再在 Pin 上保存 peer pin ids。

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
| DTO → Store | `addGraphFromData` (graphDataStore) | GraphInstanceDTO 直接 hydrate 到规范化 Store；`toStoredPin()` 剥离旧运行时 links |
| DTO → Store (连接) | `connectionItemToConnectionData` (graphConverters) | ConnectionItemDTO → ConnectionData |
| Store → DTO | `connectionDataToItem` (graphConverters) | ConnectionData → ConnectionItemDTO |
| 项目加载 | `loadProject` / `refreshResourceIndex` | 后端 ProjectIndex + graph 文档直接写入 Store |
| Pin 连接状态派生 | `derivePinConnectionView` (pinLinks) | 从 `pinConnections[pinId]` 派生 `connected` / `linkCount` / `connectionIds` |
| Spawn → IPC params | `spawnParamsToInstanceParams` (nodeInstanceParams.ts) | 创建节点打 tag |
| NodeCreated → Store | `flattenInstanceParams` (NodeEventHandler) | flatten DTO 写入 NodeData |
| Undo apply | `NodeService.applyGraphPatch` | GraphUndoPatch 透传 |

---

## 十一、注意事项

1. **inputs/outputs 双态**：DTO 和 Store 使用 Pin ID 数组；Canvas / Detail 渲染时由 `useNodeView` / `NodeDetailPanel` 从 `pins` + `pinConnections` 组装 `PinView`（含派生连接状态）。
2. **Connection 命名**：JSON 统一使用 camelCase `fromPin`/`toPin`；Store 的 ConnectionData 使用 `from`/`to` 并派生 `id`。
3. **Pin direction**：后端序列化为 `"input"`/`"output"`，与前端 PinDirection 一致。
4. **Node 命名**：Domain/Store 使用 snake_case（node_type, ui_style），DTO 使用 camelCase（nodeType, uiStyle）；转换时需注意字段映射。
5. **节点 params 与 Layout params**：`NodeInstanceParams`（tagged union）与 `EditorGroupNodeParams`（布局选中态）是两套类型，详见 [DESIGN_RULE.md §3.8](./DESIGN_RULE.md#38-节点实例参数与结构性-undo-dto)。

---

## 十二、NodeInstanceParams（节点实例参数）

Rust：`graph/node/node_instance.rs` — `#[serde(tag = "paramsKind")]`。

前端单点：`src/shared/types/dto/nodeInstanceParams.ts`。

### 12.1 变体对照

| paramsKind | Rust 变体 | JSON 字段（camelCase） | 典型节点 |
| --- | --- | --- | --- |
| `none` | `None` | `{ paramsKind: "none" }` | 普通运算 / 控制流节点 |
| `variable` | `Variable { variable_id, variable_name?, variable_type? }` | `variableId`, `variableName?`, `variableType?` | Get/Set Variable |
| `subGraph` | `SubGraph { sub_graph_id }` | `subGraphId` | Call Function |
| `dataFrame` | `DataFrame { dataframe_id }` | `dataframeId` | Get DataFrame |

### 12.2 序列化形态差异

| 载体 | 序列化 | 前端类型 |
| --- | --- | --- |
| `NodeInstanceDTO` | `#[serde(flatten)]` 展开到节点顶层 | `NodeInstanceCoreDTO & NodeInstanceParamsDTO` |
| `NodeSubgraphDTO`（undo） | 嵌套在 `instanceParams` | `instanceParams?: NodeInstanceParamsDTO` |
| `batch_create_nodes` 请求项 | `params: NodeInstanceParamsDTO \| null` | `BatchCreateNodeIpcItem` |

### 12.3 创建路径扁平参数 → tagged union

创建命令使用扁平 `NodeSpawnParams`（`variableId` / `subGraphId` / `dataframeId` 等），经 `spawnParamsToInstanceParams` 打 tag 后 invoke 后端。

事件回传 `NodeInstanceDTO` 经 `flattenInstanceParams` 写入 `NodeData` 扁平字段。

**禁止**在 `createNode.ts`、`useClipboardStore` 等 command 层重复 inline 定义 params 形状；统一 `import type { NodeSpawnParams } from '@/shared/types/dto/nodeInstanceParams'`。

---

## 十三、GraphUndoPatch（结构性 Undo）

Rust：`schema/history.rs` — `GraphUndoPatch`, `NodeSubgraphDTO`。

前端：`src/shared/types/dto/graphUndoPatch.ts`（`services/graph/node/graphUndoPatch.ts` 仅 re-export）。

### 13.1 结构

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `nodes` | `NodeSubgraphDTO[]` | 被删 / 断连涉及的主节点子图快照 |
| `neighborNodes` | `NodeSubgraphDTO[]`（可选） | 动态邻居在变更时刻的 pin 冻结 |
| `connections` | `ConnectionRebuildDTO[]` | `{ fromPin, toPin }` 连线重建列表 |

### 13.2 NodeSubgraphDTO

| 字段 | 后端 | 前端 | 说明 |
| --- | --- | --- | --- |
| id | `NodeId` | `string` | 节点 ID（undo 时保留原 ID） |
| nodeType | `String` | `string` | 节点类型名 |
| position | `NodePosition` | `NodePosition` | `{ x, y }` |
| instanceParams | `NodeInstanceParams` | `NodeInstanceParamsDTO` | **嵌套** tagged union |
| typeVarMap | `HashMap<TypeVarId, TypeVarDefinition>` | `Record<string, TypeVarDefinitionDTO>` | 类型变量绑定 |
| pins | `Vec<PinInstance>` | `SubgraphPinInstanceDTO[]` | 完整 pin 快照（含 id） |

### 13.3 SubgraphPinInstanceDTO（Pin 磁盘形态）

与运行时 `PinInstanceDTO`（画布事件用）不同，undo 使用磁盘序列化形态：

| 字段 | 说明 |
| --- | --- |
| `definition?` | 完整 `PinDefinitionDTO`（动态 / 需持久化全定义的 pin） |
| `pinContract?` | 精简契约 `{ name, direction, kind, role, optional? }`（固定 pin） |
| `order?` | Pin 排序 |
| `typeVarId?` | 关联类型变量 |
| `userValue?` | 用户输入值 |

二者**二选一**，由 Rust `PinInstance` 的 `should_persist_full_definition()` 决定。

### 13.4 数据流

```
delete / disconnect / paste redo
  → 后端 capture_subgraph / capture_disconnect_undo_patch
  → GraphUndoPatch JSON
  → 前端 history context 原样保存
  → undo: NodeService.applyGraphPatch(graphId, patch) 透传
  → 后端 apply_graph_patch 恢复节点 + 邻居 + 连线
```

空 patch 使用常量 `EMPTY_GRAPH_UNDO_PATCH`，避免各处手写 `{ nodes: [], ... }` 不一致。

### 13.5 扩展约定

- 新增 `NodeInstanceParams` 变体：同步 §十二，并确认 undo 捕获路径 `instance_params.clone()` 自动携带新字段。
- 变更 `PinInstance` 磁盘序列化：同步 `SubgraphPinInstanceDTO` 与 `PinDefinitionDTO` / `PinContractDTO`。
- 前端**不得**在 apply 前裁剪 patch 字段；若需变换，应在 Rust `apply_graph_patch` 单点处理。

---

## 十四、转换函数补充（节点 params / undo）

| 场景 | 函数/位置 | 说明 |
| --- | --- | --- |
| 创建 → IPC | `spawnParamsToInstanceParams` | `NodeSpawnParams` → `NodeInstanceParamsDTO \| null` |
| 批量创建 | `toBatchCreateNodeIpcItems` | `batchCreateNode.ts` |
| 事件 → Store | `flattenInstanceParams` | `NodeEventHandler.dtoToNodeData` |
| Undo 空快照 | `EMPTY_GRAPH_UNDO_PATCH` | `graphUndoPatch.ts` |
| Undo 空快照 | `EMPTY_GRAPH_UNDO_PATCH` | `graphUndoPatch.ts` |
| Apply undo | `NodeService.applyGraphPatch` | patch 透传，无前端重组 |

---

## 十五、InfoView 统计数值（展示层 DTO 防御）

与 §十二/十三 的 IPC 边界不同，报告数值在展示层须二次防御。单点模块：`src/views/InfoView/shared/formatStat.ts`。

| 函数 | 用途 |
| --- | --- |
| `coerceFiniteNumber` | 拒绝对象/数组，窄化为有限 number |
| `formatNum` | 通用数值字符串（含 `Inf` / `—`） |
| `formatNullableNum` | 可空字段 + 自定义 fallback |
| `formatPercent` | 0–1 比例 → 百分比字符串 |

约定详见 [DESIGN_RULE.md §2.9](./DESIGN_RULE.md#29-infoview-统计数值展示)。

---

## 十六、Info 报告 Payload（IPC 边界）

与 §十二/十三 的图节点 DTO 不同，报告 JSON 来自 `get_value` / `publish_report`，按 `ReportKind` 在边界窄化。

| 层级 | 模块 | 说明 |
| --- | --- | --- |
| 入口分发 | `parseReportPayload(report, raw)` | 覆盖全部 `ReportPayloadKind`；`null` = 无效 |
| 回归族 | `parseRegressionResultData` | OLS / WLS / GLS / Logit / Probit 等共用 |
| 面板 / VAR / VEC / DF-ADF | `parsePanel` / `parseVar` / `parseVec` / `parseDfadf` | 按模型单文件维护 |
| 共享字段 | `parseCommon` | 系数、`serialTests`、`correlogram` |
| 类型真源 | `shared/types/report/*.ts` | `InfoView/shared/types.ts` 仅 re-export |

**扩展**：新增 `ReportKind` 时同步 Rust struct、`parseReportPayload` 分支、`ReportView` 渲染分支与 `report.test.ts`。展示层数值仍须 [§2.9](./DESIGN_RULE.md#29-infoview-统计数值展示) / §十五 二次防御。

约定详见 [DESIGN_RULE.md §2.13](./DESIGN_RULE.md#213-info-报告-ipc-边界与类型分层)。

---

## 十七、Graph store hydrate

| 层级 | 模块 | 说明 |
| --- | --- | --- |
| 类型契约 | `shared/types/store/graph.ts` | `GraphData` / `GraphDataLike` / `RuntimeNodeInput` |
| normalize 单点 | `normalizeGraphDataLike` | 所有入站图数据规范化 |
| Pin 引用窄化 | `runtimePinRefToId` / `runtimePinRefsToIds` | `RuntimeNodeInput.inputs/outputs` |
| 连接兼容 | `normalizeGraphConnections` | DTO / domain / 历史快照多形态 |
| Store 写入 | `graphDataStore.buildGraphBucket` | 实体桶索引 |
| 同步入站 | `graphUpdatedPayloadToGraphDataLike` | `GraphEventHandler` |
| 测试夹具 | `makeTestGraph()` | hydrate-safe `GraphDataLike` |

约定详见 [DESIGN_RULE.md §2.14](./DESIGN_RULE.md#214-graph-store-hydrate)、[adr/graph-store-hydrate.md](./adr/graph-store-hydrate.md)。
