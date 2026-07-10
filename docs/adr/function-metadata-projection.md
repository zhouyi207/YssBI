# ADR: Function Metadata Projection (三 Store 分工)

## Status

Accepted — 2026-07-10

## Context

函数资源在前端被拆成三类元数据，各自有权威 store：

| Concern | 权威 Store | 字段 |
| --- | --- | --- |
| 资源身份 / 显示名 | `ResourceStore` | `id`, `name`, `exists`, `loaded` |
| 函数签名 | `graphMetaStore` | `functionInputs`, `functionOutputs` |
| 图体（节点 / pin / 连线） | `GraphDataStore` | `graphEntities` 桶 |

历史上还存在：

- `GraphData.functionInputs/Outputs` 作为 IPC / 导出边界字段（**不**写入 `GraphEntityBucket`）
- Detail 面板单独订阅 `graphMetaStore` 再与 `ResourceStore` 合并（第四份合并逻辑）
- `graphMetaStore.graphOrder` 与 `ResourceStore.graphOrder` 重复

## Decision

1. **UI 读路径单点**：`functionResourceView.ts` + `useFunctionCatalog()` 合并名称与签名；Detail 经 `useEditorSessionResources().functions` → `resolveDetailPanelModel`，禁止组件内再订阅 `graphMetaStore` 做合并。

2. **签名写入单点**：`functionSignatureSync.ts`（`syncFunctionSignatureFromGraph` / `hydrateFunctionSignaturesFromProjectIndex`）在 load graph、invoke 回包、FunctionCreated/Updated 事件时写入 `graphMetaStore`。

3. **导出组装单点**：`buildGraphSnapshotFromStores()` 从三 store 组装 `GraphData`（含函数签名）；`getGraphByPath` / `exportSnapshot` 均经此路径。

4. **Canvas 图体 hook**：`useGraphData` 只返回名称 + 图体，不含签名（签名由 Call 投影 / palette 经 `useFunctionCatalog` 读取）。

5. **删除 `graphMetaStore.graphOrder`**：排序以 `ResourceStore.graphOrder` 为准。

## Consequences

- 新增需要「函数名 + 签名」的 UI，使用 `useFunctionCatalog` 或 `selectFunctionResourceView`，不要手写双 store 合并。
- 新增需要完整图（含签名）的导出/序列化，使用 `buildGraphSnapshotFromStores` / `getGraphByPath`。
- `GraphData` / `GraphInstanceDTO` 仍可在 IPC 边界携带 `functionInputs/Outputs`，但入 store 后签名以 `graphMetaStore` 为准；`addGraphFromData` 必须伴随 `syncFunctionSignatureFromGraph`（已有 load / 事件 / invoke 路径）。
