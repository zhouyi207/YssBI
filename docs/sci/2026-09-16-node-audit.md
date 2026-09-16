# 2026-09-16 SCI 节点源码核对记录

> Status: Historical
> Scope: 对 355 项方法清单及当日现有节点定义、执行注册、参数和结果入口的静态核对
> Canonical owners: 本文保存当日证据快照；后续整理计划见 [README.md](README.md)，当前实现由所链接源码拥有
> Update when: 更正本次核对错误时；后续实现变化更新计划或另记有日期的核对记录

## 1. 核对来源与边界

- 日期：2026-09-16。
- 源码基线：`ed2f321106433943f2eb1e9c0212de9c37d335ad`。
- 原始文件：`docs\sci\YssBI_Method_Registry_355项.xlsx`。
- 原始文件 SHA-256：`be92fbbba7e52af2fea89df2d99a4c55210c4a98ed1d62c301230d5e0e05852e`。
- 清单统计口径：只统计 `Method Registry` 工作表中 ID 为整数的记录，跳过标题及表头。
- 方法：使用 openpyxl 只读提取清单，对照实际节点声明、内置执行注册、应用装配以及代表性的算法和界面代码。

本次没有逐项运行 355 项方法，没有开展桌面人工验收或重新执行统计 golden tests。尚未核对的算法、参数和 UI 状态保留未知；未从节点缺少注册推断算法缺失。统计节点数量与 Excel 方法条目不是同一计数单位。

后续内核 crate 迁移时维护了本文的实现链接，使其指向当前所有者；当日核对结论与本节源码基线保持不变，不能把链接维护视为重新运行验收。

## 2. 原始清单统计

共 355 项，涉及 27 个原分类。以下角色和优先级直接来自原表，尚未完成语义复核。

| Primary Role    | 条目数 |
| --------------- | -----: |
| Estimator       |    236 |
| Test            |     46 |
| Diagnostic      |     21 |
| Post-estimation |     11 |
| Transform       |     14 |
| Visualization   |     27 |
| 合计            |    355 |

| 原优先级 | 条目数 |
| -------- | -----: |
| P0       |    114 |
| P1       |    152 |
| P2       |     89 |
| 合计     |    355 |

名称与粒度需要整理：例如线性回归/OLS、二元 Logit/Logistic，以及面板模型与具体估计方法；AIC/BIC、分组回归和 AI 分析报告也需要分别确定产品形态。相关处理规则见[实施计划](README.md)。

## 3. 统计目录与内置执行器的对照

核对入口：

- [统计节点清单](../../src-tauri/crates/yss-node-catalog/src/statistics/families.rs)中的 `NODES`。
- [统计节点装配](../../src-tauri/crates/yss-node-catalog/src/statistics/mod.rs)使用节点 ID 作为对应执行实现标识。
- [内置执行注册](../../src-tauri/crates/yss-node-kernel/src/builtins/mod.rs)中的 `register_builtin_kernels`。
- [应用装配](../../src-tauri/crates/yss-application/src/session/components.rs)中的 `NodeComponents::builtins`。

当日统计目录有 25 个节点定义，其中 2 个在当前内置执行器中注册，23 个未注册。以下“已注册”仅描述装配状态，不代表本次完成运行或数值验收。

| 节点 ID                            | 目录名称                     | 当前内置执行注册 |
| ---------------------------------- | ---------------------------- | ---------------- |
| `yssbi.statistics.adf.test`        | DF / ADF 单位根检验          | 未注册           |
| `yssbi.statistics.adf.summary`     | DF / ADF 汇总                | 未注册           |
| `yssbi.statistics.ols.fit`         | 拟合 OLS                     | 已注册           |
| `yssbi.statistics.ols.summary`     | OLS 汇总                     | 已注册           |
| `yssbi.statistics.gls.fit`         | 拟合 GLS                     | 未注册           |
| `yssbi.statistics.gls.summary`     | GLS 汇总                     | 未注册           |
| `yssbi.statistics.iv.2sls.summary` | 工具变量 2SLS 汇总           | 未注册           |
| `yssbi.statistics.iv.liml.summary` | 工具变量 LIML 汇总           | 未注册           |
| `yssbi.statistics.logit.fit`       | 拟合 Logit                   | 未注册           |
| `yssbi.statistics.logit.summary`   | Logit 汇总                   | 未注册           |
| `yssbi.statistics.panel.summary`   | 面板模型汇总                 | 未注册           |
| `yssbi.statistics.panel.did.twfe`  | 面板双重差分（双向固定效应） | 未注册           |
| `yssbi.statistics.prais.fit`       | 拟合 Prais–Winsten           | 未注册           |
| `yssbi.statistics.prais.summary`   | Prais–Winsten 汇总           | 未注册           |
| `yssbi.statistics.linear.predict`  | 线性模型预测                 | 未注册           |
| `yssbi.statistics.logit.predict`   | Logit 预测                   | 未注册           |
| `yssbi.statistics.probit.predict`  | Probit 预测                  | 未注册           |
| `yssbi.statistics.probit.fit`      | 拟合 Probit                  | 未注册           |
| `yssbi.statistics.probit.summary`  | Probit 汇总                  | 未注册           |
| `yssbi.statistics.var.lag_order`   | VAR 滞后阶数选择             | 未注册           |
| `yssbi.statistics.var.summary`     | VAR 汇总                     | 未注册           |
| `yssbi.statistics.vec.fit`         | 向量误差修正模型             | 未注册           |
| `yssbi.statistics.vec.rank_test`   | Johansen 协整秩检验          | 未注册           |
| `yssbi.statistics.wls.fit`         | 拟合 WLS                     | 未注册           |
| `yssbi.statistics.wls.summary`     | WLS 汇总                     | 未注册           |

`NodeComponents::new` 允许缺少执行实现的定义保留在目录中；[图分析](../../src-tauri/crates/yss-graph-analysis/src/lib.rs)通过 `with_execution_kernel_support` 生成 `NodeKernelUnavailable` 诊断。因此目录中可见的节点不能直接计为可执行方法。

## 4. 已确认差距及对整理顺序的影响

### 4.1 已有算法需要逐项接通当前节点入口

[SCI 回归入口](../../src-tauri/crates/yss-sci/src/regression/fit.rs)可见 WLS、GLS、Logit、Probit、Prais 等实现调用；[Runtime 回归入口](../../src-tauri/crates/yss-sci-runtime/src/regression/mod.rs)还提供工具变量与面板调用；[Runtime 时间序列入口](../../src-tauri/crates/yss-sci-runtime/src/time_series/models.rs)包含 ADF、VAR、VEC 和协整秩检验相关调用。

这些代码支持“先盘点复用与接入”的顺序。每个方法仍需分别核对支持范围、参数传递、数据准备和结果契约，不能把注册缺口作为唯一剩余工作。

### 4.2 OLS 汇总的名称与实际职责需要统一

[目录端口](../../src-tauri/crates/yss-node-catalog/src/statistics/mod.rs)中的 OLS `fit_ports` 与 `summary_ports` 都接收数据。[统计执行](../../src-tauri/crates/yss-node-kernel/src/builtins/statistics.rs)中的 `OlsFit` 与 `OlsSummary` 进入同一拟合计算，再投影不同输出。

目前拟合节点输出模型记录、拟合值和残差；汇总节点的 `result` 与 `report` 共享原生 OLS 结果。整理时需要先明确拟合、模型结果和报告的连接方式，确保名称能说明行为，并支持下游复用一次拟合。

### 4.3 参数和端口也有需要同步处理的差距

- [统计节点定义](../../src-tauri/crates/yss-node-catalog/src/statistics/mod.rs)为 WLS 拟合增加了 `weights` 输入；WLS 汇总没有相应权重输入，应在接入前明确其职责。
- 同一目录为 Logit 声明了迭代次数和容差配置，但当前 [SCI 通用回归入口](../../src-tauri/crates/yss-sci/src/regression/fit.rs)使用 `LogitConfig::default()`。由于该节点尚未接入，不能声称现有界面配置已贯通计算；接入方案需要同时处理这一点。

### 4.4 选列待办已经落后于源码

当日[内置执行器](../../src-tauri/crates/yss-node-kernel/src/builtins/mod.rs)已注册 `yssbi.dataframe.series.select`，[关系执行实现](../../src-tauri/crates/yss-node-kernel/src/builtins/relational.rs)调用 `select_series`，[目录](../../src-tauri/crates/yss-node-catalog/src/dataframe/mod.rs)声明了 `NodeTypingSpec::ColumnOutput`。

原 [TODO](../../TODO.md)将选列与求和同时记为未注册，与源码不一致。此次文档整理将该待办收敛为仍未注册的 `yssbi.dataframe.series.sum`；选列的完整使用体验仍需在首批流程中运行验收。

## 5. 可以复用的体验基础

| 已有入口                                                                              | 静态核对可确认的能力                           | 后续使用方式                               |
| ------------------------------------------------------------------------------------- | ---------------------------------------------- | ------------------------------------------ |
| [节点目录](../../src-tauri/crates/yss-node-catalog/README.md)                         | 已有节点定义、分类、本地化、别名和帮助文档装配 | 在现有目录中整理名称和职责                 |
| [参数配置面板](../../src/modules/details/internal/ui/node/NodeConfigurationPanel.tsx) | 按投影中的配置字段渲染并提交图修改             | 统一变量角色、参数组织及实际传参           |
| [OLS 报告布局](../../src/shared/types/domain/olsReportSpec.ts)                        | 已定义章节种类、显示配置、结果引用和解析校验   | 作为报告体验样板，后续按模型扩展           |
| [OLS 结果分析](../../src-tauri/crates/yss-graph-execution/src/result/analysis.rs)     | 已有基于 OLS 结果的相关分析入口                | 明确哪些下游能力在首批复用                 |
| [回归 golden tests](../../src-tauri/crates/yss-sci/tests/regression_golden.rs)        | 已有数值回归测试代码和参考值                   | 修改相关算法时按范围运行，并核对参考值来源 |

上述能力的存在来自静态代码阅读，不能推导所有节点已具备相同能力。本次建议和后续任务统一维护在[SCI 节点整理与实施计划](README.md)。
