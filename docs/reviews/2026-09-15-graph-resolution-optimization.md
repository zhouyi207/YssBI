# Graph 实时解析优化记录

> Status: Historical
> Scope: 2026-09-15 对旧 Resolve 分析的源码复核、实现和本地验证
> Canonical owners: [Graph 与 Execution](../architecture/GRAPH_AND_EXECUTION.md)、相关源码及根 package scripts
> Update when: 本次验证数据需要更正时；后续行为以 canonical owner 为准

## 结论与实现

旧分析中的重复解析问题仍存在，但当前产品已经取消独立 Compile 操作，执行计划在运行时准备。本次保留编辑时更新 pins、类型、Schema 和诊断，以及 Ctrl+S 显式保存的行为，优化共用的解析与投影路径。

| 原问题                      | 已实现的处理                                                                                                                 |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| 重复 Resolve 仍重算完整语义 | Graph Runtime 复用中性的 GraphAnalysis，命中时跳过 Schema、类型和函数语义求解。                                              |
| 展示修改进入完整解析        | 节点位置和 user label 直接从当前文档投影；资源显示名、语言仍按当前请求本地化。                                               |
| Schema 每次重新求解         | 输出缓存按节点输入、上游 Schema、registry 和实际资源读取失效；上游 Schema 未变时停止向下游传播重算。                         |
| 批次每步都前置 Resolve      | 仅引用端口、需要校验或 claim 派生端口的操作请求前置解析；其余操作直接生成补丁，批次完成后发布一次投影。                      |
| 全图重复扫描和快照复制      | 解析复用借用文档的 binding、输入顺序、连线计数索引；投影按节点索引语义和诊断；本地化和 kernel 支持检查接收已有快照的所有权。 |

核心实现：

- [解析入口](../../src-tauri/crates/yss-graph-runtime/src/lib.rs)、[有界缓存](../../src-tauri/crates/yss-graph-runtime/src/semantic_cache.rs)。
- [解析身份](../../src-tauri/crates/yss-graph-document/src/semantic_hash.rs)、[资源观察](../../src-tauri/crates/yss-graph-resource-contract/src/catalog.rs)。
- [Schema 增量求解](../../src-tauri/crates/yss-graph-analysis/src/schema_resolution.rs)、[解析中的文档索引](../../src-tauri/crates/yss-graph-analysis/src/document_index.rs)。
- [编辑编排](../../src-tauri/crates/yss-application/src/graph/edit.rs)、[投影索引](../../src-tauri/crates/yss-graph-editor/src/projection/mapper.rs)。

## 缓存正确性

解析缓存不能只使用执行哈希。执行身份排除了常量名称、函数参数名称、某些 connection ID 和 orphan metadata，但快照仍保留这些内容。本次单独计算解析身份；这些字段变化会更新快照，节点布局变化则复用语义。执行哈希原有格式及输入顺序保持不变。

资源检查只读取此前的依赖集合，包括不存在的资源和传递函数正文。无关 catalog 变化可复用快照；资源出现、消失或内容变化会使相应结果失效。Schema 缓存命中时也重放资源读取记录，确保后续缓存和运行准备没有丢失依赖。

运行能力指纹参与完整解析缓存检查。缓存存储本地化和 Application kernel 支持检查之前的结果，重复请求不会累计诊断。Application 仍捕获并重验当前资源与 session，结果有效性仍使用本次数据库快照中的数据版本。

缓存只保存每图最近一次派生结果，并限制驻留图数量。计算与大对象释放不持有缓存锁；并发丢失命中机会最多导致重新计算。缓存没有当前文档、undo/redo 或文件写权限。

## 测量方法

[Rust 探针](../../src-tauri/crates/yss-graph-runtime/src/resolution_tests.rs)直接调用生产 Resolve 和 Editor Projection，使用以下两组图：

- scalar：100、1000、5000 个逻辑节点，包含未绑定输入诊断。
- dataframe：相同规模，每 10 个节点组成数据源与 Limit 链，Schema 有 12 列。

两种模式都预热节点缓存。对照模式每次仅丢弃完整快照，保留类型和 Schema 缓存；复用模式保留完整快照。它们比较的是本次实现内的完整快照复用收益，不能当作所有修改相对于旧提交的总体收益。

```powershell
pnpm test:rs:package -p yss-graph-runtime --release --lib benchmark_repeated_resolution_and_projection '--' --ignored --nocapture
```

最终数据记录在 [graph-resolution-results.json](probes/graph-resolution-results.json)。数值是单机合成场景的阶段耗时，不包含 Project 捕获与提交、结果有效性计算、Tauri 编码/传输和 React 安装、绘制；不能据此承诺端到端帧率。Debug 初步测量只用于定位重复扫描，最终对照使用 Release 构建。

Windows x64，Release，每种模式 20 次，平均耗时（ms）：

| 图形      | 节点数 | 强制丢弃完整快照：Resolve | 完整快照命中：Resolve | 命中后生成投影 |
| --------- | -----: | ------------------------: | --------------------: | -------------: |
| scalar    |    100 |                     1.039 |                 0.188 |          0.281 |
| scalar    |   1000 |                     9.658 |                 1.939 |          2.875 |
| scalar    |   5000 |                    46.741 |                 8.696 |         12.425 |
| dataframe |    100 |                     4.022 |                 0.452 |          0.290 |
| dataframe |   1000 |                    41.206 |                 4.920 |          3.266 |
| dataframe |   5000 |                   219.675 |                26.678 |         18.620 |

1000 节点 dataframe 场景的 Resolve 均值约减少 88%；加上该探针测得的投影阶段，从约 44.9 ms 降至 8.2 ms。5000 节点相同场景命中后两阶段仍约 45.3 ms，说明缓存不能代替端到端性能验收。

## 验证范围

新增回归覆盖快照复用、展示刷新、常量值与元数据、函数参数名称、传递函数正文、缺失资源恢复、运行能力指纹、诊断 connection ID、缓存淘汰、Schema 重新连线、环路、删除和表格常量单元格变化。Schema 增量结果及资源观察与全量解析逐项比较。

应用与执行测试复用真实的图编辑、Harness 批次、派生端口、undo/redo、显式保存、执行与结果读取路径。未新增 UI 单元测试；此次没有进行桌面 UI 人工验收，也没有运行全仓 CI。

本次已通过的聚焦验证：

| 范围                                                                              | 结果              |
| --------------------------------------------------------------------------------- | ----------------- |
| Graph Analysis / Runtime / Editor / Document / Resource Contract 的 Rust lib 测试 | 54 通过           |
| Application / Graph Execution 的 Rust lib 测试                                    | 109 通过          |
| Application `numeric_execution`，启用 `test-support`                              | 5 通过            |
| 上述五个 Graph 包的 Clippy，`--lib --tests --no-deps -D warnings`                 | 通过              |
| Release 性能探针                                                                  | 1 通过，12 组数据 |
| 文档契约、module map 检查、`git diff --check`                                     | 通过              |

测试使用根 `pnpm test:rs:package -p <package> --lib` 入口；数值集成使用 `pnpm test:rs:package -p yss-application --features test-support --test numeric_execution`。Graph 测试中的性能探针默认 ignored，以上命令按显式过滤单独执行它。全范围 Application Clippy 与全仓 CI 不在本次通过声明中。

全项目生产架构检查**未通过**。命令为 `pnpm test:rs:package -p yssbi --lib architecture_tests::tests::rust_production_architecture_matches_declared_policy '--' --exact`，最后失败原因为 `ExternalDependencyDeclarationSetMismatch`，unexpected 项为 `yss-harness-rig/Runtime/futures-util`。当前 [Harness manifest](../../src-tauri/crates/yss-harness-rig/Cargo.toml) 与 [外部依赖登记](../../src-tauri/src/architecture_tests/external_policy.rs) 不一致；本次 Graph 优化未修改这两处。新增文档索引的内部引用路径问题已修正，但不能据此宣称完整生产架构审计通过。

## 剩余成本

完整快照未命中时，图遍历、输入校验、节点事实组装与函数验证仍会发生。Schema 的缓存按输出复用，并不意味着整个 Resolve 仅访问受影响节点；函数正文在完整快照未命中时仍会重新验证。缓存命中也需要输入指纹、快照复制、本地化和投影生成，较大的常量或宽表 Schema 会增加这些成本。

批次中的连接必须读取前序操作改变后的端口事实，因此混合连接批次可能多次解析。这保留了自动派生 pins 和错误定位的正确性。
