# 当前图数据编辑实施记录

> Status: Historical
> Scope: Project 当前图数据、后端历史、显式保存、Harness 与读投影增量的实现和验证
> Canonical owners: 当前行为由 Graph 与 Execution、Project 及 IPC 文档维护
> Update when: 保留本次交付记录；后续行为在专项架构文档中更新

## 实现结果

GraphDocument 直接由 Rust ProjectData 持有并编辑。GraphEditingMetadata 记录可逆历史、编辑身份和已保存内容指纹，前端保留只读文档/语义投影及临时交互状态。没有独立的 Draft 文档容器，也没有前端 undo/redo 栈。

- 普通编辑、Harness、undo/redo 使用同一数据层提交边界；图文档和历史一起原子更新。
- Ctrl+S 持久化捕获的当前正文，成功后更新保存指纹。失败保留内存编辑和历史。
- 重命名保留未保存正文及其历史，文件仅写入对应的资源变更。函数签名事务同样不顺带保存未提交正文。
- 节点目录查询、执行、复制导出都以当前图版本读取数据，常规命令不再上传完整文档。
- Harness 直接调用 Application，原先依赖 Webview 的 prepare/claim/adopt 图草稿交接路径已移除。
- snapshot/delta 共享严格的投影安装入口。坏增量通过一次只读查询恢复，不重放写操作。
- 通知与命令回执合并，前端复用未变化节点、端口和连线对象。拖动过程仍本地预览，结束时提交。
- 可逆补丁的数据定义位于 GraphDocument 契约；应用与校验仍在 document-edit。Application 发布中立活动通知，Tokio/Tauri 通道留在 IPC 适配层。

主要入口：[Project 编辑状态](../../src-tauri/crates/yss-project/src/project_state/graph_editing.rs)、[Application 编辑用例](../../src-tauri/crates/yss-application/src/graph/editing.rs)、[前端投影同步](../../src/services/nodeSystem/graphEditorSync.ts)、[架构契约](../architecture/GRAPH_AND_EXECUTION.md)。

## 性能测量

[基准程序](../../src/tests/benchmarks/graphEditorSync.bench.ts) 使用生产解码器与投影 Store，覆盖 JSON 解析、增量应用和 Store 采纳；IPC 由内存模拟。样本采用同质合成节点，完整数据见 [测量输出](probes/graph-editor-sync-results.json)。

| 节点数 | 完整快照平均 / p99 | 增量平均 / p99   |
| ------ | ------------------ | ---------------- |
| 100    | 0.87 / 3.57 ms     | 0.37 / 0.89 ms   |
| 1,000  | 8.62 / 15.54 ms    | 3.94 / 5.37 ms   |
| 5,000  | 50.14 / 62.36 ms   | 26.61 / 51.42 ms |

1,000 节点的这段前端处理平均耗时降低约 54%。该测量没有包含真实 Tauri IPC、Rust Resolve、DOM 绘制和真实业务图的 Schema 复杂度，不能作为桌面帧率保证。5,000 节点仍出现约 50 ms 的尾部处理时间。

复现命令：`pnpm bench:graph --outputJson docs/reviews/probes/graph-editor-sync-results.json`。

## 自动验证

| 检查                                                      | 结果                                                                                                           |
| --------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Project、GraphDocument、document-edit、Application 库测试 | 132 项通过，包含真实后端历史恢复结果缓存                                                                       |
| 前端服务、解析、FIFO、结果协调与项目身份契约              | 90 项通过                                                                                                      |
| 前端架构检查                                              | 52 项通过                                                                                                      |
| Rust 真实生产依赖检查                                     | 通过；新边界只授予所需的具体类型与映射入口                                                                     |
| TypeScript 类型检查                                       | 通过                                                                                                           |
| 文档契约                                                  | 6 项通过                                                                                                       |
| `git diff --check`                                        | 通过                                                                                                           |
| TypeScript lint                                           | 完成，保留其他模块已有告警                                                                                     |
| Rust 严格 Clippy                                          | 未通过；SCI 依赖及既有命令参数数量、事件枚举尺寸等告警仍存在。本次新增通道代码的类型复杂度与条件折叠问题已修复 |

Rust 库检查使用 `pnpm test:rs:package -p yss-project -p yss-graph-document -p yss-graph-document-edit -p yss-application --lib`。架构检查使用 `pnpm test:architecture` 及 Root crate 的 `rust_production_architecture_matches_declared_policy`。这不是完整 workspace CI。

数值执行集成检查 `pnpm test:rs:package -p yss-application --features test-support --test numeric_execution` 的 5 项用例也已通过，覆盖项目数据集、图解析与分页结果的真实链路。

## 人工验收

按仓库规则，未新增 UI 单元测试。实际桌面仍需检查：

1. 拖动、输入、撤销与重做的反馈，以及保存后关闭重开。
2. GUI 和 Harness 交替编辑时的刷新、冲突反馈与统一历史。
3. 未保存时重命名、修改函数签名、外部文件刷新及选择“不保存”关闭。
4. 在代表性业务图和 release 构建下测量输入延迟、帧率、峰值内存与大图表现。

JSON 语义报告 Renderer 的试点仍是后续独立工作；本次交付聚焦当前图的数据编辑与投影链路。
