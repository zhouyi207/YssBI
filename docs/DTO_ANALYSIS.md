# 前后端 DTO 数据结构分析与统一方案

## 一、当前数据结构对比

### 1. Node（节点）

| 字段 | 后端 NodeInstanceDTO | 前端 Domain Node | 前端 graphDataStore NodeData | 说明 |
|------|---------------------|------------------|------------------------------|------|
| id | ✅ NodeId | ✅ string | ✅ string | 一致 |
| node_type | ✅ | ✅ | ✅ | 一致 |
| category | ✅ Vec<String> | ✅ string[] | ✅ | 一致 |
| title | ✅ | ✅ | ✅ | 一致 |
| inputs | ✅ Vec<String> (Pin IDs) | ❌ Pin[] (完整对象) | ✅ string[] (Pin IDs) | **不一致** |
| outputs | ✅ Vec<String> (Pin IDs) | ❌ Pin[] (完整对象) | ✅ string[] (Pin IDs) | **不一致** |
| ui_style | ✅ | ✅ | ✅ | 一致 |
| description | ✅ Option | ✅ optional | ✅ | 一致 |
| position | ✅ NodePosition | ❌ 在 UINode | ✅ { x, y } | Domain Node 无 position |
| graphPath | ❌ | ❌ | ✅ | Store 专用，用于索引 |

### 2. Pin（针脚）

| 字段 | 后端 PinInstanceDTO (camelCase) | 前端 Domain Pin | 前端 Store PinData | 说明 |
|------|--------------------------------|-----------------|---------------------|------|
| id | ✅ | ✅ | ✅ | 一致 |
| nodeId | ✅ node_id (snake) | ✅ nodeId | ✅ nodeId | 命名需统一 |
| name | ✅ | ✅ | ✅ | 一致 |
| type | ✅ (rename pin_type) | ✅ type | ✅ type | 一致 |
| direction | ✅ | ✅ | ✅ | 一致 |
| links | ✅ Vec<PinId> | ✅ string[] | ✅ string[] | 一致 |
| defaultValue | ✅ default_value | ✅ defaultValue | ✅ | camelCase |
| userValue | ✅ user_value | ✅ userValue | ✅ | camelCase |
| isArray | ✅ is_array | ✅ isArray | ✅ | camelCase |
| ui | ✅ | ✅ | ✅ | 一致 |

### 3. Connection（连接）

| 字段 | 后端 ConnectionItemDTO | 前端 Domain ConnectionItem | 前端 Store ConnectionData | 说明 |
|------|------------------------|----------------------------|---------------------------|------|
| from_pin | ✅ | ✅ from_pin | ❌ from | **不一致** |
| to_pin | ✅ | ✅ to_pin | ❌ to | **不一致** |
| id | ❌ 无 | ❌ 无 | ✅ "from->to" | Store 派生 ID |

### 4. Graph（图）

| 字段 | 后端 GraphInstanceDTO | 前端 Domain Graph | 说明 |
|------|----------------------|-------------------|------|
| id | ✅ | ✅ | 一致 |
| name | ✅ | ✅ | 一致 |
| type | ✅ graph_type (enum) | ✅ type (string) | 需统一 |
| nodes | ✅ NodeInstanceDTO[] | ✅ Node[] | Node 结构需统一 |
| pins | ✅ PinInstanceDTO[] | ✅ Pin[] | 一致 |
| connections | ✅ ConnectionDTO | ✅ Connection | 结构一致 |
| canvas | ✅ GraphPosition | ✅ GraphPosition | 一致 |

---

## 二、核心问题

1. **Node inputs/outputs 双态**：后端和 Store 用 Pin ID 数组；Domain/序列化用完整 Pin 对象
2. **Connection 命名**：后端/序列化用 `from_pin`/`to_pin`；Store 用 `from`/`to` + 派生 `id`
3. **缺失类型定义**：`NodeData`、`PinData`、`ConnectionData`、`GraphData` 未在 shared/types 中定义
4. **Node 的 type vs node_type**：历史视图转换曾同时兼容 `n.type` 和 `n.node_type`，现已收口到 Store hydrate / 视图选择器路径

---

## 三、统一方案

### 3.1 分层类型定义

```
┌─────────────────────────────────────────────────────────────┐
│  DTO 层（前后端传输）                                         │
│  - NodeInstanceDTO, PinInstanceDTO, ConnectionItemDTO        │
│  - 与后端 Rust 结构 1:1 对应，snake_case 序列化               │
└─────────────────────────────────────────────────────────────┘
                              ↕ 转换
┌─────────────────────────────────────────────────────────────┐
│  Domain 层（业务逻辑）                                        │
│  - Node, Pin, Connection, Graph                              │
│  - 统一使用 inputs/outputs 为 Pin ID 数组（与后端一致）         │
└─────────────────────────────────────────────────────────────┘
                              ↕ 转换
┌─────────────────────────────────────────────────────────────┐
│  Store 层（前端状态）                                         │
│  - NodeData, PinData, ConnectionData                         │
│  - 规范化存储，ConnectionData 含 id/from/to                    │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 统一字段规范

**Node**
- `inputs`: string[] (Pin IDs)，前后端统一
- `outputs`: string[] (Pin IDs)，前后端统一
- `position`: 必选，后端已有

**Connection**
- DTO/序列化: `{ from_pin, to_pin }`
- Store: `{ id: "from->to", from, to }`，id 由 from+to 派生

**Pin**
- 统一 camelCase（前端），后端 DTO 使用 `#[serde(rename_all = "camelCase")]` 已支持

---

## 四、已实现的类型定义

### 4.1 新增文件

| 路径 | 说明 |
|------|------|
| `src/shared/types/domain/ids.ts` | NodeId, PinId, GraphId, ConnectionId, VariableId |
| `src/shared/types/dto/graph.ts` | NodeInstanceDTO, PinInstanceDTO, ConnectionItemDTO, ConnectionDTO, GraphInstanceDTO |
| `src/shared/types/store/graph.ts` | NodeData, PinData, ConnectionData, GraphData |

### 4.2 使用指南

```typescript
// 后端 DTO（接收/发送）
import type { NodeInstanceDTO, PinInstanceDTO, ConnectionItemDTO, GraphInstanceDTO } from '@/shared/types/dto/graph';

// Store 层（graphDataStore）
import type { NodeData, PinData, ConnectionData, GraphData } from '@/shared/types';
import type { NodeId, PinId, GraphId, ConnectionId } from '@/shared/types';

// 连接格式转换
import { connectionItemToConnectionData, connectionDataToItem } from '@/shared/utils/editor/dtoConverters';
```

### 4.3 注意事项

1. **NodeCreated 事件**：当前仅添加节点到 Store，pins 需由后续 Graph 同步或后端扩展 NodeCreated payload 包含 pins
2. **Connection 双格式**：addGraphFromData 已兼容 `{ from_pin, to_pin }` 与 `{ id, from, to }`
3. **Pin direction**：后端 Rust enum 可能序列化为 "Input"/"Output"，前端使用 "input"|"output"，必要时做大小写转换
