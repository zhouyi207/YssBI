# yss-sci-runtime

> Status: Current
> Scope: 同步科学计算入口、中立数值输入与 SCI 调用边界
> Canonical owners: 本 crate 源码拥有适配与入口；数值算法和线性代数由对应 SCI crates 拥有
> Update when: 计算入口、输入输出契约或调用边界改变时

Stateless scientific computation entry points over `yss-sci`, called directly
by `yss-node-kernel` and `yss-graph-execution`'s focused SCI benchmark.

Runtime calls SCI with ordinary vectors, slices and contract records. It has no
faer or `yss-sci-linalg` dependency and does not construct numerical matrices or own
estimators. SCI converts numeric inputs into Linalg matrices and returns computed results.
Tabular transformations and time/panel alignment use native relation plans in
`yss-database-engine`, requested by [Node Kernel](../yss-node-kernel/README.md).
Runtime receives prepared neutral inputs and has no Arrow dependency.

`regression::linear::linear_regression` and `time_series::acf_pacf` accept neutral requests and `ScientificExecutionControl` from
`yss-sci-contract`. Fit and Summary kernels forward the execution cancellation and deadline.

`acf_pacf` applies the report policy (at least four observations and a positive requested
lag, capped at `min(n / 2 - 1, 40)`), then passes slices and the same control to
`yss-sci::time_series::acf_pacf::compute_acf_pacf`. SCI owns finite-input validation and the
joint numerical calculation: one ACF feeds the PACF recursion. It checks cancellation
and deadlines throughout input/numerical loops and before returning. The runtime
rechecks control and rejects nonfinite ACF/PACF coefficients before delivering the shared `AcfPacfResult`; no duplicate runtime request/result
records are maintained. SCI itself permits lags through `n - 1`; 40 is a report budget.
Application queries retained results and does not depend on SCI runtime. Desktop composition
does not call it or construct/inject a backend object.
The runtime has no Execution dependency.
Linear regression checks cancellation and deadlines during input validation, before
SCI dispatch, before report projection and before returning the result;
these checks do not promise cooperative interruption of a running matrix decomposition.

## Capability modules

`visualization` exposes controlled, stateless plot-data computations over neutral
slices and model facts. Algorithms stay in SCI, tabular preparation in Node Kernel,
and rendering in D3. It does not create or retrieve results.

Domain names follow [SCI's category mapping](../yss-sci/README.md#domain-organization).
Entry points and method-specific report records live in their owning domains.

| Module                 | Responsibility                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------------- |
| `regression`           | Regression-family entry points and result serialization                                                     |
| `regression::linear`   | Controlled OLS/WLS/GLS and configurable Prais computation, linear model records                         |
| `regression::discrete` | Configurable Logit/Probit fits and prediction from existing coefficients                                |
| `regression::models`   | Stateless exports of controlled robust, penalized, GLM/likelihood, nonlinear and model-building entries |
| `regression::report`   | Report labels, fields and serialization                                                                 |
| `hypothesis`           | Neutral hypothesis and margins `at()` entry points into SCI                                             |
| `anova`                | Controlled ANOVA/ANCOVA, MANOVA and repeated-measures entry points                                      |
| `association`          | Correlation, partial/rank correlation and inter-rater agreement entry points                            |
| `multivariate`         | Controlled multivariate analyses with separate scores and summaries                                     |
| `time_series`          | ACF/PACF, ADF, VAR and VEC entry points                                                                 |
| `diagnostics`          | Residual/model and serial-correlation tests, neutral diagnostic records                                 |
| `panel`                | Panel fitting and selected report projections                                                           |
| `causal::iv`           | IV fitting, selected diagnostics and report projections                                                 |
| `causal::did`          | TWFE DID fitting and randomization inference                                                            |
| `descriptive`          | Gini, Dagum decomposition and Theil T entry points                                                      |
| `distribution`         | Probability distribution sampling entry point                                                           |

There is no empty `SciContext` or parallel `api/backends/rust` route. Capability
entry points call the corresponding SCI owner; report encoding stays here.

`regression::models` exposes SCI's neutral functions directly. It adds no estimator,
options mirror or report model; node adapters supply aligned columns, budgets,
category coding and label restoration. All additional regression analyses return
the shared structured contracts for the existing result inspection flow.

`multivariate` exposes controlled PCA, principal-axis factors, canonical correlation,
correspondence, LDA/QDA, RDA and classical MDS entry points and their neutral summaries
and separate row-major scores. It neither rebuilds numerical models nor introduces
another result hierarchy; Kernel owns exact labels and coordinate-table conversion.

`anova::{anova, manova, repeated_measures}` exposes SCI's controlled neutral
functions and shared `yss-sci-contract::anova` reports directly. Runtime does not
construct designs, refit models or duplicate ANOVA inference. Node adapters own
tabular alignment, budgets and original factor-label restoration.
Fit contracts come from `yss-sci-contract`; Runtime does not maintain a second
hierarchy of regression, panel or diagnostic result types.

`descriptive::gini` and `descriptive::dagum_gini` forward neutral slices and execution
control to SCI and return its shared descriptive contracts. They do not recalculate
statistics or introduce another report model; category labels are restored by the
node adapter. Theil retains its form and observation-count projection here.

`association` forwards neutral slices, options and execution control to SCI for
Pearson/partial/Spearman/Kendall, Kappa, ICC, Bland–Altman, Kendall W, Ridit and rwg.
It returns the shared contracts directly; node adapters own ordinal/category
interpretation and label restoration, while Results owns storage and rendering.

## Linear regression data flow

`LinearRegressionRequest.options` and the algorithm's configuration use the same
`yss_sci_contract::regression::OlsOptions`. The Graph catalog reads its OLS defaults
from that contract. The runtime passes the selected options to the model without
an intermediate configuration mirror.

SCI's `regression::linear::fit::fit_linear_regression` selects OLS/WLS/GLS and projects the fit, including its already-computed fitted
values and residuals. `regression::report::linear_regression_report` returns typed model and
coefficient statistics without duplicating observation arrays. The `linear_regression` function
returns these statistics alongside ordinary fitted/residual vectors, fitted
design columns and the original optional WLS weights. Residual diagnostics reuse
these weights; they never silently substitute an unweighted fit. The Summary kernel computes the selected ACF/PACF, serial-test and
hypothesis analyses from that model. Execution retains the shared model and immutable
summary; Application validates result identity and reads those computed analyses.
Serial-test input/output
records belong to `yss-sci-contract::diagnostics::serial_correlation`. No opaque JSON report crosses
this boundary.

Binary and Prais entry points accept their shared options instead of rebuilding defaults.
IV accepts separate exogenous, endogenous and instrument column collections, preserving
fitted values, residuals, coefficient inference and the design needed for later analyses.
`causal::iv::summary` requests first-stage, overidentification and endogeneity analyses
only when selected. Panel accepts `PanelOptions`; SCI selects the estimator/effect
combination and owns all matrix construction. Panel Summary projects the selected
model, coefficient, effect and estimator statistics from the shared fit contract.
VAR/VEC Summary likewise selects report contents and calls SCI for requested
serial tests, lag exclusion or stability. Granger, IRF and FEVD are independent
postestimation calls; IRF/FEVD take their horizon at that stage. Summaries reuse the
fitted model without refitting it, and fits do not cache full reports.
`causal::did::randomization_test` accepts observed data and execution control, fits the
observed TWFE specification once and checks cancellation between permutations.

Kernel density plot preparation uses `visualization::kde`, which passes the caller's
execution control to SCI's density computation.

The crate does not own project/database state, graph scheduling, result storage,
Tauri commands, frontend state, Julia processes or Bayesian worker lifecycle.

## 宿主调用与数值所有权

科学计算使用独立的中性契约。Node Kernel 负责拟合与汇总分析，Execution 仅在独立 OLS benchmark 中直接调用 runtime；IPC Command 通过 Application 读取已计算的结果：

```text
Application → yss-graph-execution → yss-node-kernel → yss-sci-runtime (stateless functions)
OLS benchmark (dev dependency) → yss-sci-runtime
yss-sci-runtime → yss-sci algorithms → yss-sci-linalg Mat / Col / views / checked factors → faer
yss-sci algorithms → shared model options/results in yss-sci-contract
Plugin Manager → framed IPC → Julia extension → Bayes worker port → Julia adapter
```

`yss-node-kernel` 拥有中立调用契约、运行值及冻结注册表；Application 装配后向 Execution 注入同一注册表。图计划、资源授权、结果存储和节点错误定位仍由 Execution/Application 的现有所有者负责，kernel 不反向依赖 Graph、Project 或 Application。具体调用边界见 [Node Kernel](../yss-node-kernel/README.md)。

Rust algorithms 拥有统计数值和 typed result；React 只把 authoritative DTO 转换为 presentation model。Julia process/runtime、Bayes model validation、worker protocol、artifact 和 result 随独立插件编译；`yss-bayes-runtime` 是插件内部的科学编排，不是宿主 bridge。宿主 Application 只实现通用数据快照和结果提交端口，不包含 Julia/Bayes 专用 command。已提交插件结果位于项目 `extension-results/`，包含内容哈希、包摘要、操作身份、输入快照来源和通用文件；卸载插件不删除它们。

[`yss-sci-linalg`](../yss-sci-linalg/README.md) 拥有不透明的 `Mat`、`Col`、行与借用视图，以及矩阵运算、分解检查、稳定错误类型和秩阈值。faer 仅是该 crate 的实现依赖，对外不重导出原生类型。SCI 只通过 Linalg 使用矩阵；runtime 只调用 SCI，不依赖 Linalg 或 faer。中性契约使用业务结构和普通向量，Arrow 负责表格交换；输入与报告按逻辑行列转换，不依赖矩阵物理存储顺序。

[`yss-sci`](../yss-sci/README.md) 的 OLS 模块分别组织模型/结果、拟合和推断，使用中性契约中的唯一 `OlsOptions`。Runtime 按 regression、hypothesis、time_series、panel、causal、diagnostics 等领域组织入口；表格准备由 Node Kernel 与数据库引擎承担，runtime 与核心算法均不依赖 Arrow 或 Polars。线性回归报告由 runtime 映射 OLS/WLS/GLS 拟合结果，预测值和残差使用模型已计算的事实。`regression::linear::linear_regression`、`time_series::acf_pacf` 是普通函数，接收中性请求和取消/deadline 控制；桌面入口和 Application 不构造、保存或注入科学计算后端。

SCI 拥有数值设计矩阵、回归拟合、ADF/VAR/VEC 模型准备、DID 随机化推断和核密度计算。假设检验也归 SCI：复用 `yss-math-expr` 解析，完成约束线性化、参数列序、矩阵构造与 t/Wald 分派；Application 保留结果身份和项目状态检查。Julia 插件不依赖任何 SCI crate，输入值、分类角色和取消/期限契约由插件内的 `yss-bayes-worker` 拥有。

`report_display` assembles bounded declarative section metadata pointing to ordinary
result paths. Flat named coefficient, first-stage, response/impulse and covariance
rows remain scientific result data; Application owns allowed presentation bindings
and lazy table paging. Formula strings are backend formatting, not frontend math.
Section and equation-size limits are explicit, and array shape mismatches fail.
Binary Summary, Prais, IV, panel, ADF and VAR/VEC use this current report path.
