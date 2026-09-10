# 数据引擎迁移验收

> Status: Historical
> Scope: 2026-09-10 工作区的 DuckDB 与宿主 Polars 替换验收
> Canonical owners: 源码、测试和 manifests 拥有实现事实；本文保存验收证据，不替代当前架构文档
> Update when: 修正本次验收记录时；后续行为变化更新对应 canonical owner

后续同日用户要求继续替换插件 Polars；当前插件已采用 DataFusion/Arrow，详见 [Julia 插件](../../plugins/julia/README.md)。
本文保留第一阶段宿主迁移的验收快照，以下“插件保留 Polars”描述仅适用于该阶段。

依据 [迁移分析](../architecture/迁移.md) 的 A–E 工作包，以及
[DataFusion 方向](../architecture/DATAFUSION.md)、[Polars 边界分析](../architecture/POLARS.md)，
宿主已切换为 DataFusion 关系执行、Arrow 数据边界、Parquet 数据文件和 SQLite 数据集 catalog。
原有 Project、Graph compiler、GraphSemanticSnapshot、历史、ScientificBackend 和 ResultStore 保留其职责。

当前行为由 [Database runtime](../../src-tauri/crates/yss-database-runtime/README.md)、
[Dataset store](../../src-tauri/crates/yss-dataset-store/README.md)、
[Graph / Execution](../architecture/GRAPH_AND_EXECUTION.md) 和
[系统架构](../architecture/ARCHITECTURE.md) 说明。

## 逐项验收

| 要求                                        | 实现与验证证据                                                                                                                                                                                   | 结论 |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---- |
| A：精确存储 Schema 与图语义类型分离         | `yss-database-schema` 保持中性 facts；`yss-tabular-arrow` 保留整数宽度/符号、Decimal 精度/scale、时间单位/时区、NULL、ColumnId 和类别域。Arrow 与根数据库回归验证 UInt64、Decimal、时间及导出。  | 通过 |
| A：宿主数据边界无损，分页 DTO 可用          | IPC/CSV/Parquet 使用原生 Arrow；超过 JavaScript 安全范围的整数按十进制字符串显示。外部字典按标签重映射，实际类别域有独立预算。CSV 宽文本、带换行字段和超大记录有回归。                           | 通过 |
| B：Compile 与中间关系节点不整表物化         | Source/Project/Filter/Series/Limit/Rename 组合原生计划；集成测试在 Parquet 文件尚不存在时成功 Compile 和构造关系，创建文件后才执行统计读取。                                                     | 通过 |
| B：统计需求列联合读取、样本对齐及缺失值策略 | Series 必须来自同一个关系句柄；OLS 共同投影后消费 Arrow 批流，使用独立输入预算，拒绝 NULL/非有限值。多批次、反向输入顺序和筛选测试核对响应、解释变量、拟合值及残差。                             | 通过 |
| B：真实项目资源授权与结果发布               | CSV 导入后的真实 Project Graph 经 Compile/Execute、原 ScientificBackend、finalization 和 ResultStore 完成 OLS；发布前重新验证捕获的 Project grants。                                             | 通过 |
| B：固定快照、有界 Results、取消与迟到拒绝   | Results 保存关系句柄及文件租约；分页保留未知总数、hasMore 和精确列类型。查询控制覆盖取消、deadline、预算；Application 测试拒绝失效后的在途页面成功和失败。                                       | 通过 |
| B：语义与能力检查权威不分裂                 | GraphSemanticSnapshot 继续拥有类型、Schema、lineage 和诊断；Compile 查询真实 KernelRegistry，阻断未实现 kernel。Execution/Graph 聚焦回归及完整 Rust 架构门禁通过。                               | 通过 |
| C：仅 committed catalog 决定可见数据集      | SQLite 保存身份、head、Schema、修订、文件成员和交接记录；激活不以目录扫描登记数据集。重开恢复原内容，失败准备和提交冲突不改变旧快照。                                                            | 通过 |
| C：外部导入与内部存储分开                   | `DatabaseImportSource` 保留 CSV/Parquet/Excel/SQL；持久化 `DatabaseEngine` 只保留 Dataset。SQLx 使用有背压的 Arrow 批次，Excel 沿用 calamine owner。                                             | 通过 |
| D：稳定行列身份、显示顺序与编辑查询         | RowId 单调分配，DisplayOrder 独立；ColumnId 不随改名变化。类型化稀疏覆盖通过存在标记区分显式 NULL；过滤和 limit 作用于合并后的视图。插行、删除、筛选进出及跨文件身份回归通过。                   | 通过 |
| D：Undo/Redo、Save、cast 与 compaction 分离 | 既有 EditHistory 持有前后快照；cast/压实写新 generation，撤销恢复原值。Save 清历史并建立检查点，不清持久化 Delta，也不默认重写整表；Delta 超预算在一次原子发布内压实。                           | 通过 |
| D：提交前后故障及文件生命周期               | 文件关闭/同步后才提交 SQLite CAS 和 publication 记录；提交前可回收准备，提交后按 journal 恢复，结果不确定时保留文件。活动头、Undo、查询和待发布记录保护旧文件；清理队列可重试。                  | 通过 |
| D：完整项目生命周期与格式切换               | 创建、导入、重命名、编辑、另存为、关闭/重开、删除及 registry/recovery 失败路径通过 Project/Application 测试。格式 5 使用新 catalog；格式 4 在创建 catalog 前拒绝，旧文件保持不变。               | 通过 |
| D：另存为与路径边界                         | 复用 Project filesystem transaction，Parquet 和其他大文件流式复制；64 MiB 文件首尾内容及复制后数据集重开有回归。catalog/generation 路径拒绝 symlink 和 Windows reparse point，清理不跟随重定向。 | 通过 |
| E：清理旧宿主依赖和适配器                   | `yss-duckdb`、`yss-tabular-polars` 源码/manifests/成员及权限已删除；宿主生产和测试夹具不再使用 Polars。SCI 数据准备采用 Arrow，数值核心不依赖 DataFusion。                                       | 通过 |
| E：插件边界与依赖兼容                       | 仅 Julia extension、Bayes worker 和 artifact adapter 内部保留 Polars。三个 crate 的编译检查通过；宿主插件快照夹具改用 Arrow，native_extension 测试目标编译通过。                                 | 通过 |
| E：性能验收                                 | 百万行首次/重复预览、窄列/全列扫描、高选择性筛选、编辑后查询、cast/compaction、OLS 输入与矩阵阶段分别测量，见[测量记录](../reference/DATA_ENGINE_BENCHMARK.md)。                                 | 通过 |

首条关系执行闭环采用迁移分析明确指定的 Source → Project → Filter → Series → OLS。
Join、全部时间序列、其他统计模型和函数子图执行属于该文档明确保留的后续范围；未实现能力在 Compile 被拒绝。
Julia 插件内的 Polars 独立保留，不构成第二个宿主关系执行权威。

## 验证记录

命令均在仓库根目录执行，Rust 使用固定工具链。以下是最终受影响范围的验证，未将聚焦检查称为完整 CI。

| 命令/范围                                                                                                     | 结果                                                                    |
| ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-dataset-store -p yss-project -p yss-application -p yss-api --lib`                | 127 项通过：13 / 32 / 52 / 30；覆盖 catalog、生命周期、恢复、发布和 IPC |
| `pnpm test:rs:package -p yssbi --lib architecture_tests`                                                      | 86 项通过，包含真实 production source/依赖/语义门禁                     |
| `pnpm test:rs:package -p yssbi --test database_test`                                                          | 5 项通过，包含编辑失败、类型/撤销、重开、导出及大文件另存为             |
| `pnpm test:rs:package -p yss-application --test numeric_execution`                                            | 5 项通过，包含惰性 Parquet 和真实 Project Graph → OLS → Results         |
| `pnpm check:rs:package -p yss-julia-extension --bin yss-julia-extension`                                      | 通过，包含两个内部 Polars 适配器                                        |
| `pnpm check:rs:package -p yss-plugin-runtime --test native_extension`                                         | 编译通过；未运行需打包插件/Julia 的 ignored 用例                        |
| `pnpm lint:rs:package -p yss-dataset-store --tests '--' -D warnings`                                          | 通过，包含新增路径和异步调用边界                                        |
| `pnpm check:ts`、`pnpm lint:ts`                                                                               | 通过；Oxlint 仍报告工作区已有的 14 条 warning                           |
| `pnpm test:ts` 定向选择数据库编辑窗口 identity/readOnly、Results 分页 hook/renderer 和 resultService 五个文件 | 9 项通过                                                                |
| `pnpm test:ts src/tests/architecture/documentationContract.test.ts`                                           | 6 项通过                                                                |
| `pnpm generate:diagnostics:check`、`pnpm docs:module-map:check`                                               | 通过                                                                    |
| `git diff --check`、变更文件格式检查                                                                          | 通过                                                                    |

同一任务内较早的有效结果还包括 Arrow、关系适配器、SQL 输入、数据库 runtime/edit、Execution/Graph
相关库测试，SQL/CSV 批流与类别域回归；SCI `time_series_operations` 14 项、linalg contracts 5 项、
SCI `regression_golden` 3 项、Project filesystem 47 项均通过。受影响数据库管理与项目发布的前端五个文件
34 项通过。相关实现未再发生行为变化的结果复用，不为累计测试数量重复运行。

数据库库测试曾暴露同步 catalog 在 Tokio 生命周期内部嵌套 runtime 的 panic；现对这种调用使用
scoped thread，8 个原失败生命周期用例全部通过。完整架构测试也已移除旧依赖/源码布局断言，
以真实分类和权限数据验证新边界；仅基准装配点获得 SCI 实现的明确访问权限。

依赖核查使用 `cargo tree --manifest-path src-tauri/Cargo.toml -p yssbi -e normal --prefix none`
及源码/manifests 搜索：宿主无 DuckDB/Polars，workspace lock 中的 Polars 仅服务插件。
DataFusion、Arrow、Parquet 和插件 Polars 版本以 workspace manifests/Cargo.lock 为准。

## 限制与独立问题

- 格式 4 项目不会自动转换；打开时明确拒绝并保留原文件。此迁移不提供新旧引擎切换或兼容存储路径。
- `pnpm test:ts src/features/core/chart/chartDocumentStore.test.ts` 仍为 2 项通过、3 项失败：
  原有 Chart 夹具没有 mock `get_project_index`，触发 Node 环境中的真实 Tauri IPC 与 `window is not defined`。
  该测试文件未改动，失败属于工作区原有项目发布改动，本次未修改对应 Chart 产品行为。
- 更宽的 SCI Clippy 检查存在原有 lint；本次记录的存储/查询与文件边界 Clippy 范围通过，未宣称 workspace Clippy 全绿。
- 未执行 `pnpm run ci`、桌面手动交互、打包安装及真实 Julia 插件推断，也未连接外部 MySQL/PostgreSQL 实例。
  SQL 映射/批流以本地 SQLite 与类型契约测试验证，插件结果限于编译兼容性。
- 性能为 debug 构建的单次本地观察，操作系统文件缓存未清空；不据此宣称比旧引擎更快或给出 release 延迟保证。
