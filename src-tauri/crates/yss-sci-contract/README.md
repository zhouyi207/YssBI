# yss-sci-contract

> Status: Current
> Scope: 按 SCI 领域组织的中立输入、选项、结果及共享执行控制
> Canonical owners: 本 crate 源码拥有科学计算契约；算法与报告组装由 SCI 和 Runtime 拥有
> Update when: 中立契约、领域归属或执行控制改变时

Backend-neutral scientific contracts shared by application workflows, Execution,
the scientific runtime and numerical models.

Domain names follow [SCI's category mapping](../yss-sci/README.md#domain-organization).
Cross-domain execution and observation contracts remain shared.

- `execution`: `ScientificExecutionControl`, cancellation and computation errors.
  These types contain no execution-plan or project identities, and define no backend trait.
- `anova`: coded categorical factors, bounded design dimensions, factorial and
  sums-of-squares options, univariate/ANCOVA tables, MANOVA tests/SSCP and
  within-subject tables with Greenhouse–Geisser correction. Generic factor labels
  allow adapters to restore exact tabular values; optional MANOVA inference fields
  explicitly represent unavailable small-sample F approximations.
- `regression`: the single OLS configuration/default and covariance selection.
  `linear` defines OLS/WLS/GLS computation requests/results; `fit` defines neutral
  regression statistics; `report` defines typed OLS
  summary records. Report construction and labels belong to the runtime.
  `summary` owns the selected linear-summary contents and analysis defaults; it
  does not contain UI layout, graph identity, or cache state.
  `discrete::BinaryOptions` owns binary-model intercept/convergence settings;
  `prais` owns AR(1) transform and convergence settings. Linear results retain
  optional original WLS weights for downstream diagnostics.
  `models` owns bounded predictor/category dimensions, estimator/convergence
  options and structured coefficient/model/workflow results for the additional
  regression methods. Method-specific facts use `RegressionDetails`; undefined
  inference/metrics use optional fields. Generic category/group labels preserve
  original scalar identities, and workflows retain original predictor/row indices.
- `hypothesis`: neutral hypothesis requests, results, alternatives and errors.
- `multivariate`: PCA, principal-axis factors, canonical correlation, simple CA,
  LDA/QDA, RDA and classical MDS options and typed summaries. Numerical observation
  coordinates remain separate row-major outputs. Classification labels are generic
  for exact adapter restoration. Bounds cover variables, classes, table categories
  and dense MDS points; undefined inference/proportions use explicit optional fields.
- `association`: paired correlation, partial-correlation, rank-test and inter-rater
  agreement options/results. It distinguishes Pearson confidence options from rank
  inference options, six ICC definitions, Cohen/Fleiss weighting, and rwg null models.
  Generic category labels preserve caller-owned scalar identities; unavailable
  inference uses `Option`, never NaN. Bounds cover raters, controls, categories,
  exact permutations and display points.
- `time_series::acf_pacf`: ACF/PACF requests and results. `var` and `vec` own
  neutral fitted models and selected-summary options; `fit` shares multivariate
  equation/coefficient statistics, serial-test rows and stability roots.
- `diagnostics::serial_correlation`: serial-correlation requests and BG/Q/Durbin-Watson results.
- `diagnostics::residual`: residual/model test selection and BP/White/IM/RESET,
  normality, VIF and leverage results without numerical-backend types.
  VIF uses `None` for the intercept's undefined VIF/tolerance, preserving explicit
  optional values rather than encoding missing statistics as NaN.
- `panel`: estimator/effect options, `PanelFit`, selected-summary options and
  grouped coefficient/model/effect statistics. Estimator-specific statistics use
  an enum instead of unrelated optional fields on every result.
- `causal::iv`: IV estimator selection and fitted model facts, including design
  columns for later analyses. First-stage, overidentification, Hausman and
  endogenous-regressor results are independent records, selected through
  `IvSummaryOptions` rather than embedded in every fit.
- `causal::did`: DID inputs, inference results and typed unavailable/error codes.
  Randomization input contains observed columns, treatment/post indicators,
  repetitions and a reproducible seed; interruption remains an explicit failure.
- `density`: kernel-density input and output records.
- `visualization`: typed XY/reference lines, histograms, grouped distributions,
  intervals, ROC/AUC, category frequencies, rectangular matrices and coefficient
  intervals. Display bounds and sampling metadata contain no pixel layout,
  graph identity or result/window lifecycle.
- `descriptive`: empirical `GiniResult`, Dagum group/pair/component statistics, and the
  64-group report bound. Group labels are generic so the node adapter can replace
  numerical group IDs with original scalar labels without changing statistics.
- `distribution`: probability distribution parameters and typed samples.
- `observation`: observation metadata, missing-value policy and category roles,
  exported from the crate root for all domains.
- `error`: stable operation and scientific error vocabulary.

This crate owns data and execution-control contracts, not algorithms, report rendering,
project/database state, Tauri, Polars, faer or concrete backend implementations.
Observation metadata records row selection counts and the applied missing-value
policy. There is no global approximate-equality tolerance configuration.

Bayesian plugin inputs and cancellation controls belong to the plugin's
`yss-bayes-worker`; plugins do not depend on this crate.

`ScientificExecutionControl::check` gives cancellation priority over deadline expiry.
ACF/PACF checks during input scans, ACF accumulation and PACF recursion; linear
regression checks between stages, without interrupting a running decomposition.
Scheduling, concurrency and the budget remain caller-owned.

`hypothesis` carries neutral requests and result records for classical mean, proportion, count-table, rank/sequence and variance-homogeneity tests. Requests encode design and alternatives; the report keeps the statistic, reference degrees of freedom, p-value, sample sizes and method-specific finite details without depending on a node ID or backend type.

Binary effect/classification records retain nullable inference for undefined rates;
regression fits retain named numeric designs for postestimation. Panel estimation
samples explicitly describe transformed coordinates and source-row groups. VAR
records retain selected lags, covariance divisor, exogenous names and sample rows.
These neutral records do not contain graph addresses or UI-specific types.
