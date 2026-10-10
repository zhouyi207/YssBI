# 基准与测量数据

> Status: Current
> Scope: Rust 基准入口、保留的历史测量与 GPUI 性能验收边界
> Canonical owners: 基准源码定义测量方法；原始结果限定记录时的环境和范围
> Update when: 测量方法、命令入口或数据文件改变时

保留的结果是特定环境中的观察，不代表当前 GPUI 桌面端到端性能。复测时记录提交、构建配置、数据规模和机器环境；历史原始输出不作为当前源码的验证证据。

## Rust 基准

| 测量 | 方法与入口 | 既有结果 |
| --- | --- | --- |
| 数据引擎 | [查询、编辑、压实与 OLS 测量说明](DATA_ENGINE_BENCHMARK.md) | [历史 JSON](DATA_ENGINE_BENCHMARK_RESULTS.json) |
| Graph 实时解析 | [Rust 基准](../../crates/yss-graph-runtime/src/resolution_tests.rs)：合成图的语义快照复用与投影生成 | [历史 JSON](probes/graph-resolution-results.json) |

从仓库根目录显式运行 Graph 的手工性能用例：

```sh
cargo test -p yss-graph-runtime --release --lib benchmark_repeated_resolution_and_projection -- --ignored --nocapture
```

原测量的 scalar 图包含 100、1,000、5,000 个逻辑节点；DataFrame 图每 10 个节点组成数据源与 Limit 链，Schema 有 12 列。两种模式都预热节点缓存：对照模式丢弃完整快照，复用模式保留完整快照；类型和 Schema 缓存都保留。

该比较只说明语义快照复用的局部收益，不包含 Project 提交、结果有效性或 GPUI 绘制，不代表所有优化相对旧版本的总体收益。当前图与报告契约见 [Graph application](../../crates/yss-application/src/graph/README.md)。

## GPUI 性能验收

使用 [GPUI host](../../crates/yss-desktop-gpui/README.md) 的原生运行入口，在相同图规模、窗口大小与显示器刷新率下测量输入、平移、缩放、拖动和连接。业务基准不能代替真实窗口的帧率和响应验收。开放项目见 [GPUI 功能计划](../roadmap/GPUI_MIGRATION.md)。

## 历史参考数据

以下记录测量 `react/` 参考实现的 JSON 解码、投影安装、Store 采纳或快照准备，不测量 GPUI。原记录中的前端工具命令仅用于识别当时的测量环境，不能作为当前仓库根目录的运行指令；相关构建入口不再维护。

| 历史范围 | 参考源码与记录 |
| --- | --- |
| Graph 编辑同步 | [参考基准](../../react/src/tests/benchmarks/graphEditorSync.bench.ts)、[原始 JSON](probes/graph-editor-sync-results.json) |
| 已安装图的完整快照 | [初始测量](probes/graph-existing-snapshot-2026-10-02.txt)、[重排身份对齐](probes/graph-existing-snapshot-aligned-2026-10-02.txt) |
| 常量镜像与共享 | [空常量](probes/graph-constants-mirror-2026-10-02.txt)、[非空常量对照](probes/graph-constants-immer-2026-10-02.txt)、[typed 外壳](probes/graph-constants-typed-immer-2026-10-02.txt) |
| 动态键与 JSON 边界 | [合法动态键](probes/graph-constants-dynamic-keys-2026-10-02.txt)、[dataValue 边界](probes/graph-constants-data-value-2026-10-02.txt)、[投影 JSON 边界](probes/graph-projection-json-2026-10-02.txt) |
| 投影拓扑 | [typed 拓扑](probes/graph-typed-topology-2026-10-02.txt) |
| 项目快照准备 | [参考基准](../../react/src/tests/benchmarks/projectSnapshot.bench.ts)、[原始 JSON](probes/project-snapshot-results.json) |

这些记录的样本、算法、准备步骤和计时边界并不相同。顺序进程、GC、机器负载及源码变化限制了横向比较；不从异环境或异实现均值推断当前桌面提速，也不把局部测量当作架构例外的永久依据。完整环境、输出与具体局限以各记录为准。

[返回文档索引](../README.md)
