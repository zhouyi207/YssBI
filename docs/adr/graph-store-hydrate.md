# ADR: Graph Store Hydrate（`GraphDataLike` / `RuntimeNodeInput`）

## 状态

已采纳（2026-07-08）

## 背景

图数据从多条路径进入前端 store：IPC `GraphInstanceDTO`、domain `Graph` 项目快照、编辑器 `replaceGraphNodes`、同步事件 `GraphUpdated`。若各路径自行拼节点/pin/连接形态，易与 store 权威格式 `GraphData` 漂移。

## 决策

### 1. Store 权威格式

- **权威类型**：`GraphData` + 实体桶 `GraphEntityBucket`（`nodes`/`pins`/`connections`/`pinConnections` 分表）
- **连接真源**：图级 `connections` + `pinConnections`；pin 上废弃 `links` 在 `toStoredPin` 剥离，**不得**作为 hydrate 输入

### 2. 入站联合 `GraphDataLike`

`hydrateGraphs` / `addGraphFromData` 只接受 `GraphDataLike`，**必须**经 `normalizeGraphDataLike(graphPath, graph)` 再 `buildGraphBucket`。

| 入站 | 典型来源 |
| --- | --- |
| `GraphInstanceDTO` | `get_graph`、图同步事件 |
| `Graph` | `loadProject`、`domainGraphRecordToGraphData` |
| `GraphDataInput` | `replaceGraphNodes` 重组 |
| `GraphData` | 已规范化快照 |

### 3. `RuntimeNodeInput`

编辑器运行时节点允许 `inputs`/`outputs` 为 **PinId 或完整 Pin 对象**。normalize 统一为 `string[]`（`runtimePinRefsToIds`）。

`replaceGraphNodes` 是独立路径：除 normalize 外，还会从嵌入 Pin 对象与现有 store 连接重建 `pins`/`connections` 后再 `addGraphFromData`。

### 4. 出站转换

| 方向 | 函数 |
| --- | --- |
| Store → domain | `graphDataToDomainGraph` |
| domain → Store | `domainGraphToGraphData` |
| DTO → Store | `graphInstanceDtoToGraphData` → `normalizeGraphDataLike` |

### 5. 测试约定

store / sync / canvas 相关测试优先 `makeTestGraph()`（`@/tests/helpers/graphFixtures`），产出 hydrate-safe 的 `GraphDataLike`。

## 实现位置

| 职责 | 模块 |
| --- | --- |
| 类型契约 | `src/shared/types/store/graph.ts` |
| normalize 单点 | `src/shared/types/dto/graphModel.ts` — `normalizeGraphDataLike` |
| Store 写入 | `src/features/core/dataStore/graphDataStore.ts` — `buildGraphBucket` |
| 测试夹具 | `src/tests/helpers/graphFixtures.ts` |

## 反模式

| 反模式 | 原因 |
| --- | --- |
| 在 store 外手写 `nodes[].inputs = pinObjects` 直接写入桶 | 绕过 normalize，pin 表与 node 索引不一致 |
| 依赖 pin.`links` 恢复连接 | 已废弃；应用 `connections` |
| 测试内联最小 `{ id, nodes: [] }` 且不经过 hydrate | 与生产路径不一致，易假绿 |

## 相关文档

- [DESIGN_RULE.md §2.14](../DESIGN_RULE.md#214-graph-store-hydrate)
- [DTO_TYPE_MAPPING.md §十七](../DTO_TYPE_MAPPING.md#十七graph-store-hydrate)
