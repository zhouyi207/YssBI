# 基准与测量数据

> Status: Current
> Scope: 数据引擎、Graph 编辑同步与实时解析的测量资料和复现入口
> Canonical owners: 基准源码定义测量方法；原始结果限定记录时的环境和范围
> Update when: 测量方法、命令入口或数据文件改变时

保留的结果是特定环境中的观察，不代表当前桌面端到端性能。再次测量时记录提交、构建配置、数据规模和机器环境；原始输出不随文档清理重新生成。

| 测量           | 源码与方法                                                                                                                           | 结果                                               |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------- |
| 数据引擎       | [查询、编辑、压实与 OLS 测量说明](DATA_ENGINE_BENCHMARK.md)                                                                          | [原始 JSON](DATA_ENGINE_BENCHMARK_RESULTS.json)    |
| Graph 编辑同步 | [同步基准](../../src/tests/benchmarks/graphEditorSync.bench.ts)：完整快照与增量的 JSON 解码、投影安装和 Store 采纳，IPC 使用内存模拟 | [原始 JSON](probes/graph-editor-sync-results.json) |
| Graph 实时解析 | [Rust 基准](../../src-tauri/crates/yss-graph-runtime/src/resolution_tests.rs)：标量和 DataFrame 合成图的语义快照复用与投影生成       | [原始 JSON](probes/graph-resolution-results.json)  |

## Graph 编辑同步

从仓库根目录运行：

```powershell
pnpm bench:graph --outputJson docs/benchmark/probes/graph-editor-sync-results.json
```

原记录使用 100、1,000、5,000 节点合成图。结果不包含真实 Tauri IPC、Rust Resolve、DOM 绘制或业务图的完整 Schema 复杂度。

## Graph 实时解析

从仓库根目录显式运行忽略的性能用例：

```powershell
pnpm test:rs:package -p yss-graph-runtime --release --lib benchmark_repeated_resolution_and_projection '--' --ignored --nocapture
```

原测量的 scalar 图包含 100、1,000、5,000 个逻辑节点；DataFrame 图每 10 个节点组成数据源与 Limit 链，Schema 有 12 列。两种模式都预热节点缓存：对照模式丢弃完整快照，复用模式保留完整快照；类型和 Schema 缓存都保留。

该比较只说明完整语义快照复用的局部收益，不包含 Project 提交、结果有效性、Tauri 传输或 React 绘制，也不是所有优化相对旧版本的总体收益。

当前图与报告契约见 [Graph 与 Execution](../../src-tauri/crates/yss-application/src/graph/README.md)，完成记录与待验收事项见 [v0.3](../roadmap/v0_3.md)。

[返回文档索引](../README.md)
