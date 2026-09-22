# yss-sci-runtime

> Status: Current
> Scope: 同步科学计算入口、Arrow 输入准备与 SCI 调用边界
> Canonical owners: 本 crate 源码拥有适配与入口；数值算法和线性代数由对应 SCI crates 拥有
> Update when: 计算入口、输入输出契约或调用边界改变时

Stateless scientific computation entry points over `yss-sci`, called directly
by `yss-node-kernel`, `yss-graph-execution`'s focused SCI benchmark,
and `yss-application::ipc::commands`.

Runtime calls SCI with ordinary vectors, slices and contract records. It has no
faer or `yss-sci-linalg` dependency and does not construct numerical matrices or own
estimators. Arrow remains the tabular exchange representation; SCI converts
numeric inputs into Linalg matrices and returns computed results.

Time alignment consumes Arrow `Int64` and `Date32` arrays directly. Alignment and
panel preparation share the checked numeric-grid helpers in `data::time_series::align`.

`ols` and `acf_pacf` accept neutral requests and `ScientificExecutionControl` from
`yss-sci-contract`. Fit and Summary kernels forward the execution cancellation and deadline;
standalone IPC ACF/PACF uses a 60-second deadline.
Application declares this dependency for its IPC commands. Desktop composition
and other application modules do not call it or construct/inject a backend object.
The runtime has no Execution dependency.
Linear regression checks cancellation and deadlines during input validation, before
SCI dispatch, before report projection and before returning the result;
these checks do not promise cooperative interruption of a running matrix decomposition.

## Capability modules

| Module               | Responsibility                                                        |
| -------------------- | --------------------------------------------------------------------- |
| `computation`        | Implementation of the exported OLS/ACF functions and admission checks |
| `regression`         | SCI fit entry points and result serialization                         |
| `regression::report` | Report labels, fields and serialization                               |
| `regression::types`  | Existing report/model records used by capability APIs                 |
| `hypothesis`         | Neutral hypothesis and margins `at()` entry points into SCI           |
| `time_series`        | ACF/PACF, serial tests, ADF, VAR and VEC entry points                 |
| `panel`              | Entry point into SCI DID randomization inference                      |
| `data`               | Arrow panel/time alignment and tabular transformations                |
| `density`            | Density computation entry point                                       |

There is no empty `SciContext` or parallel `api/backends/rust` route. Capability
entry points call the corresponding SCI owner; report encoding stays here.

## OLS data flow

`OlsRequest.options` and the algorithm's configuration use the same
`yss_sci_contract::regression::OlsOptions`. The Graph catalog reads its OLS defaults
from that contract. The runtime passes the selected options to the model without
an intermediate configuration mirror.

SCI's `regression::fit::fit_ols` projects `OlsFit`, including its already-computed fitted
values and residuals. `regression::report::ols_report` returns typed model and
coefficient statistics without duplicating observation arrays. The OLS function
returns these statistics alongside ordinary fitted/residual vectors and the fitted
design columns. The Summary kernel computes the selected ACF/PACF, serial-test and
hypothesis analyses from that model. Execution retains the shared model and immutable
summary; Application validates result identity and reads those computed analyses.
Serial-test input/output
records belong to `yss-sci-contract::serial_tests`. No opaque JSON report crosses
this boundary.

Tabular preparation accepts Arrow arrays and `RecordBatch` values. Time alignment preserves
Int64/Date32 and column metadata, fills gaps with nulls, and checks duplicate/null times and
the bounded output size before allocating a complete grid. Lag, difference, percentage change,
and rolling means retain their numeric/null policies. Panel batch adapters reuse the existing
`align_panel` and `panel_diff` routines; they preserve entity/date types at the Arrow boundary.
These routines prepare already admitted inputs and do not own relational plans or project data.

The crate does not own project/database state, graph scheduling, result storage,
Tauri commands, frontend state, Julia processes or Bayesian worker lifecycle.

## 宿主调用与数值所有权

科学计算使用独立的中性契约。Node Kernel 负责拟合与汇总分析，Execution 仅在独立 OLS benchmark 中直接调用 runtime，IPC Command 提供独立统计命令：

```text
Application → yss-graph-execution → yss-node-kernel → yss-sci-runtime (stateless functions)
OLS benchmark (dev dependency) → yss-sci-runtime
IPC Command → yss-sci-runtime
yss-sci-runtime → yss-sci algorithms → yss-sci-linalg Mat / Col / views / checked factors → faer
yss-sci algorithms → shared model options/results in yss-sci-contract
Plugin Manager → framed IPC → Julia extension → Bayes worker port → Julia adapter
```

`yss-node-kernel` 拥有中立调用契约、运行值及冻结注册表；Application 装配后向 Execution 注入同一注册表。图计划、资源授权、结果存储和节点错误定位仍由 Execution/Application 的现有所有者负责，kernel 不反向依赖 Graph、Project 或 Application。具体调用边界见 [Node Kernel](../yss-node-kernel/README.md)。

Rust algorithms 拥有统计数值和 typed result；React 只把 authoritative DTO 转换为 presentation model。Julia process/runtime、Bayes model validation、worker protocol、artifact 和 result 随独立插件编译；`yss-bayes-runtime` 是插件内部的科学编排，不是宿主 bridge。宿主 Application 只实现通用数据快照和结果提交端口，不包含 Julia/Bayes 专用 command。已提交插件结果位于项目 `extension-results/`，包含内容哈希、包摘要、操作身份、输入快照来源和通用文件；卸载插件不删除它们。

[`yss-sci-linalg`](../yss-sci-linalg/README.md) 拥有不透明的 `Mat`、`Col`、行与借用视图，以及矩阵运算、分解检查、稳定错误类型和秩阈值。faer 仅是该 crate 的实现依赖，对外不重导出原生类型。SCI 只通过 Linalg 使用矩阵；runtime 只调用 SCI，不依赖 Linalg 或 faer。中性契约使用业务结构和普通向量，Arrow 负责表格交换；输入与报告按逻辑行列转换，不依赖矩阵物理存储顺序。

[`yss-sci`](../yss-sci/README.md) 的 OLS 模块分别组织模型/结果、拟合和推断，使用中性契约中的唯一 `OlsOptions`。Runtime 按 regression、hypothesis、time_series、panel 等能力组织入口；`runtime::data` 使用 Arrow 数组与批次完成有界时间序列/面板输入准备，runtime 与核心算法均不依赖 Polars。线性回归报告由 runtime 映射 OLS/WLS/GLS 拟合结果，预测值和残差使用模型已计算的事实。`linear_regression`、`ols`、`acf_pacf` 是普通函数，接收中性请求和取消/deadline 控制；桌面入口和 Application 不构造、保存或注入科学计算后端。

SCI 拥有数值设计矩阵、回归拟合、ADF/VAR/VEC 模型准备、DID 随机化推断和核密度计算。假设检验也归 SCI：复用 `yss-math-expr` 解析，完成约束线性化、参数列序、矩阵构造与 t/Wald 分派；Application 保留结果身份和项目状态检查。Julia 插件不依赖任何 SCI crate，输入值、分类角色和取消/期限契约由插件内的 `yss-bayes-worker` 拥有。
