# 数据引擎测量记录

> Status: Current
> Scope: 2026-09-10 写入编码、有序前缀读取和编辑过滤优化后的本地测量
> Canonical owners: `src-tauri/crates/yss-application/examples/dataset_engine_bench.rs` 与实际运行输出
> Update when: 数据规模、构建配置或数据引擎实现改变后重新测量

本次在 Windows、Intel Core i9-13900HX（24 核、32 逻辑处理器）、约 32 GiB RAM 上运行。
使用仓库固定 Rust 工具链和 debug 构建。每组先生成数据，再由独立进程测量；完整测量使用新的目录。
操作系统文件缓存未清空，“首次”指测量进程第一次查询，不表示物理磁盘冷缓存。

下表使用最终代码的三次完整测量（原始记录 run 4–6）的中位数，并列出这三次的观察范围。
测量期间另有 Cargo 进程运行，整机背景负载未隔离；这些观察不构成延迟分位数保证。
[原始 JSON](DATA_ENGINE_BENCHMARK_RESULTS.json) 同时保留首轮 run 1–3，避免只选择较快结果。
首轮之后补充了嵌套 LIMIT 的执行策略保护，基准的查询形状相同。

数据为 1,000,000 行、16 列 Float64，另有内部 RowId 和 DisplayOrder。
本次生成的 Parquet 文件均为 92,602,382 字节（约 88.3 MiB）。
DataFusion 共用 512 MiB 算子内存预算，常规查询最多 8 分区；
满足条件的单文件小前缀查询使用 1 分区。

| 场景                    |   中位数 |          观察范围 | 核对结果                       |
| ----------------------- | -------: | ----------------: | ------------------------------ |
| 数据生成与提交          |   4.13 s |     3.99 s–4.44 s | 1,000,000 行、16 列            |
| 打开 catalog 并构造关系 |  16.5 ms |   16.3 ms–25.2 ms | 不读取数据行                   |
| 首次预览                |  39.1 ms |   36.5 ms–50.6 ms | 1,000 行，仍有后续页           |
| 重复预览                |  33.3 ms |   29.8 ms–38.8 ms | 同一固定快照                   |
| 窄列全量扫描            | 919.8 ms | 915.1 ms–991.9 ms | 2 列、1,000,000 行             |
| 全列扫描                |   1.78 s |     1.71 s–1.82 s | 16 列、1,000,000 行            |
| 未编辑过滤，2 列        |  33.2 ms |   30.3 ms–34.2 ms | 999 行                         |
| 未编辑过滤，16 列       |  62.8 ms |   58.2 ms–72.8 ms | 999 行                         |
| 单个稀疏编辑提交        |  37.3 ms |   34.3 ms–43.4 ms | 修改一个单元格                 |
| 编辑后过滤，16 列       |  97.7 ms |  94.3 ms–105.2 ms | 1,000 行，首行 c0=1001         |
| 编辑后过滤，2 列        |  49.0 ms |   48.8 ms–52.2 ms | 1,000 行，首行 c0=1001         |
| OLS 联合输入准备        |  24.5 ms |   23.0 ms–25.2 ms | 100,000 行、响应和一个解释变量 |
| OLS 矩阵与统计计算      | 283.7 ms | 111.2 ms–393.0 ms | 100,000 个拟合值               |
| Compaction              |   4.91 s |     4.81 s–5.12 s | 整表重写，核对首尾 RowId 和值  |
| 持久化单列 cast         |   5.24 s |     5.03 s–5.25 s | 整表重写，Float64 → Float32    |

OLS 统计计算本次为 111–393 ms，波动明显；该计算阶段实现未改。
背景负载未隔离，因此这些数据不足以判断其是否存在性能回归。输入准备阶段三次均为约 23–25 ms。

## 本次实现与测量口径

- Float32/Float64 列关闭 Parquet 字典，采用 BYTE_STREAM_SPLIT + ZSTD；列精度、NULL、特殊浮点位模式和 Arrow metadata 由读回测试核对。
- Dataset Store 的每个 generation 文件按 DisplayOrder、RowId 有序。DataFusion 接收这一保证后可消除重复排序；投影后的单文件小前缀可下推 LIMIT。普通外部 Parquet 不假定有序，过滤、编辑覆盖、多文件或大偏移查询保留常规并行策略。
- 稀疏编辑视图按 RowId 分为未编辑与编辑分支，只有编辑分支经过覆盖值 JOIN；原生 UNION 允许过滤下推到未编辑基数据。显式 NULL、插入、删除、行顺序和快照隔离保持原有语义。
- 编辑前后分别测量相同的 2 列和 16 列。四项过滤均从构造过滤条件开始计时，包含规划与扫描；快照关系构造在计时之外。扫描计数并核对首个 c0 值，避免仅凭返回行数声称编辑正确。
- 过滤数据的 c0 按行递增，条件 c0 > 999 特别有利于 Parquet 剪枝。这不是乱序数据的一般过滤基准。
- OLS 输入来自原固定快照，核对两列长度及解释变量首尾值；compaction/cast 后在计时之外核对首尾 RowId、修改值及 cast 类型。
- Compaction 和持久化 cast 均包含完整 generation 写入、文件同步和 catalog 提交。cast 的整表重写成本仍然存在，不能将此时间解释为纯数值转换耗时。

相较同日原记录，compaction 从 14.55 s 降至约 4.91 s，持久化 cast 从 14.59 s 降至约 5.24 s，
OLS 输入从 401.1 ms 降至约 24.5 ms。原编辑后过滤记录为 426.6 ms（16 列），本次约 97.7 ms；
该项旧计时还包含快照关系构造，仅作参考对照。新口径下，16 列过滤为未编辑约 62.8 ms、编辑后约 97.7 ms。
原记录为单次观察，新记录为三次中位数，两者都不是与旧引擎的性能比较。

## 内存

采样循环每次休眠 50 ms，通过 `System.Diagnostics.Process` 读取 WorkingSet64 和 PrivateMemorySize64，
另保留操作系统报告的 PeakWorkingSet64。实际采样间隔可能更长，也可能遗漏短峰值；每个测量进程设置 2 GiB private bytes 外部停止保护。

最终三组生成进程采样峰值工作集约 33.6–35.8 MiB；
测量进程约 187.6–189.4 MiB，private bytes 约 232.1–232.9 MiB。
窄列/全列扫描的最大输出批次缓冲区仍约 128 KiB / 1 MiB。
输出批次大小不代表整个查询的峰值内存，512 MiB 算子预算也不是整个进程工作集上限。

## 复现与验证

以下命令为一组测量。每次完整重复都使用新的空目录，因为 measure 会提交编辑、压实和转换：

```powershell
pnpm exec cargo run --manifest-path src-tauri/Cargo.toml -p yss-application --example dataset_engine_bench -- D:/Temp/yss-bench-new generate
pnpm exec cargo run --manifest-path src-tauri/Cargo.toml -p yss-application --example dataset_engine_bench -- D:/Temp/yss-bench-new measure
```

程序逐场景输出 JSON。内存采样在构建结束后直接启动
`src-tauri/target/debug/examples/dataset_engine_bench.exe`，不采样 Cargo 构建进程。
release 测量需在相同入口增加 `--release` 并重新生成数据；本文未运行 release。

本次聚焦验证覆盖：

- `pnpm test:rs:package -p yss-datafusion -p yss-tabular-io -p yss-dataset-store --lib`：4 / 8 / 14 项通过。
- `pnpm test:rs:package -p yss-datafusion -p yss-dataset-store -p yss-database-runtime --lib`：最终查询改动及多文件逆序枚举回归，4 / 14 / 9 项通过。
- `pnpm test:rs:package -p yss-application --test numeric_execution`：5 项通过，包含真实 Project Graph → OLS → Results。
- `pnpm lint:rs:package -p yss-datafusion -p yss-tabular-io -p yss-dataset-store -p yss-relational-contract --lib --tests '--' -D warnings`：通过。
- `pnpm lint:rs:package -p yss-application --example dataset_engine_bench --no-deps '--' -D warnings`：example 聚焦检查通过。包含依赖的同一检查被未修改的 yss-sci 既有 67 项 lint 错误阻断。
- debug example 构建及完整复测通过；未运行 workspace 完整 CI、桌面端交互或 release 基准。

恢复、精确类型、取消和发布一致性依靠相关功能测试验证，以上性能测量不替代这些契约。
