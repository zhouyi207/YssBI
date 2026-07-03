# Runtime Source 生命周期

本文档定义 `ResultSourceStore` 中 inspectable 结果（DataView / pin 结果 / 弹窗）的创建、保留与失效规则。与 [`DESIGN_RULE.md`](DESIGN_RULE.md) 的 CQRS 原则一致：**backend 为真源，前端 `pinResults` 为投影**。

## 1. 两类 Owner

| Owner | 含义 | 索引 |
|-------|------|------|
| `RuntimePin { graphId, pinId, runId }` | 画布 output pin 上次 run 的可检视结果 | `(graphId, pinId) → sourceId` |
| `Window` | Plot / Info / RuntimeView 等弹窗独占 payload | 仅 `sourceId` |

## 2. RuntimePin 规则

| 事件 | 行为 |
|------|------|
| **Run 开始** | `clear_runtime_graph(graphId)` — 整图 runtime pin 失效 |
| **Run 结束** | **保留** — 用户继续看 Detail、Canvas overlay、embedded preview |
| **拓扑破坏** | `invalidate_runtime_pins(graphId, pinIds)` — 按 pin 失效 |
| **Undo restore**（`apply_graph_patch` 等） | **不失效** — 拓扑恢复，上次 run 结果仍可能有效 |
| **项目 unload / new / save-as** | `clear_all()` |

拓扑破坏包括（不区分用户手动断线 vs resolver 清边）：

- `PinChangeSet.removed_pin_ids`（dynamic pin strip）
- 删除节点时该节点全部 pin

实现：`RuntimeSourcesInvalidated` project-event → 前端 `clearPinResults(graphId, pinIds)`。

## 3. Window Source 规则

| 事件 | 行为 |
|------|------|
| Run 中 `insert_window_source` | 创建，`SourceOwner::Window` |
| View 复用 upstream `runtime_*` source | **不** transfer ownership |
| **窗口关闭 / unmount** | `release_window_source(sourceId)` — **仅当 owner == Window** 时删除 |
| 拓扑 invalidate RuntimePin | **不影响** Window owner（弹窗仍可看上次 run 快照） |

## 4. 前端投影

- `useExecutionStore.pinResults`：RuntimePin descriptor 索引，由 **`RuntimeSourcesInvalidated`** 驱动按 pin 删除
- `markGraphDirty`：**仅**重置执行可视化（高亮、status、recording），**不**清 `pinResults`
- 结构性改图仍调用 `markGraphDirty` 标记执行态 stale；source 清理由 backend event 负责

## 5. 与 Run 内缓存的区别

| 层 | 模块 | 生命周期 |
|----|------|----------|
| Run 内中间值 | `ExecutionDataStore` | 单次执行，GraphRuntime 内 |
| 可检视结果 | `ResultSourceStore` | 会话级，按上文规则 |
| UI 索引 | `pinResults` | 前端投影 |

## 6. 未来扩展（未实现）

- Window source LRU / TTL
- 拓扑破坏时级联 invalidate 下游 consumer output pin（当前仅 removed pin + deleted node pins）

## 7. 手动回归清单

1. Run → 查看 pin 结果 → 断开 decompose input → pin 结果消失（前后端一致）
2. Ctrl+Z 恢复连线 → pin 结果仍保留（若 run 未重跑）
3. 打开 Plot 窗口 → 关闭 → `get_result_source_descriptor` 返回 null（Window owner）
4. View 复用 upstream runtime source 的窗口 → 关闭不删除 upstream runtime pin source
