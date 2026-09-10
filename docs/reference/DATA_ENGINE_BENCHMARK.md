# 数据引擎测量记录

> Status: Current
> Scope: 2026-09-10 数据引擎迁移的本地时间与内存测量
> Canonical owners: `yss-application/examples/dataset_engine_bench.rs` 与实际运行输出
> Update when: 数据规模、构建配置或数据引擎实现改变后重新测量

本次在 Windows、Intel Core i9-13900HX（24 核、32 逻辑处理器）、约 32 GiB RAM 上运行。
使用仓库固定 Rust 工具链和 debug 构建；测量进程与生成进程分开，操作系统文件缓存未清空。
“首次”表示新进程第一次执行该查询，不表示物理磁盘冷缓存。结果不用于宣称比旧引擎更快。

数据为 1,000,000 行、16 列 Float64，分批生成；Parquet 文件共 148,897,459 字节。
DataFusion 共用 512 MiB 算子内存预算，并行度按预算限制为 8。时间为单次观察，非统计分位数。

| 场景 | 时间 | 核对结果 |
| --- | ---: | --- |
| 数据生成与提交 | 15.60 s | 1,000,000 行、16 列 |
| 打开 catalog 并构造关系 | 23.3 ms | 不读取数据行 |
| 首次预览 | 134.9 ms | 1,000 行，仍有后续页 |
| 重复预览 | 120.2 ms | 同一固定快照 |
| 窄列全量扫描 | 827.4 ms | 2 列、1,000,000 行 |
| 全列扫描 | 1.42 s | 16 列、1,000,000 行 |
| 高选择性过滤 | 21.8 ms | 999 行 |
| 单个稀疏编辑提交 | 38.0 ms | 修改一个单元格 |
| 编辑后过滤 | 426.6 ms | 1,000 行，编辑行正确进入结果 |
| OLS 联合输入准备 | 401.1 ms | 100,000 行、响应和一个解释变量 |
| OLS 矩阵与统计计算 | 91.5 ms | 100,000 个拟合值 |
| Compaction | 14.55 s | 重写 1,000,000 行并保持行身份 |
| 单列 cast | 14.59 s | 1,000,000 行，Float64 → Float32 |

以 50 ms 间隔采样单个进程：生成阶段峰值工作集约 43.9 MiB；测量阶段峰值工作集
332.3 MiB、private bytes 363.1 MiB。采样可能漏掉更短峰值，内存预算也不等于整个进程的 RSS 上限。
窄列/全列扫描观察到的最大批次缓冲区分别约 128 KiB / 1 MiB。

测量曾发现并修复两处问题：秩诊断调用完整 SVD，使 100,000 行设计矩阵申请约 80 GB
左奇异向量；现复用仅奇异值接口，保留秩阈值与统计计算。默认 32 分区的排序预留也曾耗尽
512 MiB 预算；现按内存预算确定查询并行度。高瘦矩阵回归和 OLS/WLS 黄金测试均已通过。

复现时使用一个新的空目录，先生成，再用独立进程测量：

```powershell
pnpm exec cargo run --manifest-path src-tauri/Cargo.toml -p yss-application --example dataset_engine_bench -- D:/Temp/yss-bench-new generate
pnpm exec cargo run --manifest-path src-tauri/Cargo.toml -p yss-application --example dataset_engine_bench -- D:/Temp/yss-bench-new measure
```

测量会在该目录提交编辑、压实和转换，因此再次完整测量应重新生成数据。
程序逐场景输出 JSON。内存采样在构建结束后直接启动 `target/debug/examples/dataset_engine_bench.exe`，
通过 `System.Diagnostics.Process` 读取 WorkingSet64/PrivateMemorySize64；测量设置了 2 GiB 的外部停止保护。

独立的根数据库回归还验证了 64 MiB 文件的流式另存为、首尾内容与复制后数据集重开。
这些测量覆盖指定场景，不替代恢复、精确类型、取消与发布一致性的功能测试。
