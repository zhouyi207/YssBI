# yss-sci-contract

`survival` defines neutral right-censoring curves/tests, Cox models, AFT fits,
subgroup contrasts and evaluation plots. `CoxResult` is serializable and decodable
for the nomogram consumer; prediction tables and row-domain identity remain with
Kernel/Graph. Covariance and ratio inference retain their estimator's meaning.

> Status: Current
> Scope: 按 SCI 领域组织的中立输入、选项、结果及共享执行控制
> Canonical owners: 本 crate 源码拥有科学计算契约；算法与报告组装由 SCI 和 Runtime 拥有
> Update when: 中立契约、领域归属或执行控制改变时

Backend-neutral scientific contracts shared by application workflows, Execution,
the scientific runtime and numerical models.

Domain names follow [SCI's category mapping](../yss-sci/README.md#domain-organization).
Cross-domain execution and observation contracts remain shared.

`decision/preferences` owns compact NPS/KANO summaries and row-level RFM score contracts.
Its sibling `market`, `hierarchy`, `influence`, `fuzzy` and `experts` modules own
price intersections, exact reach summaries, judgment weights, influence/reachability
cells, membership composition and single-round Delphi results respectively.
`decision/conjoint` owns centered part-worths, nullable inference and row-level rating predictions.
`psychometrics` owns scale summaries, nullable item statistics, tail-group scores
and expert CVI summaries. `multivariate::FactorabilityReport` owns shared KMO,
per-item MSA and Bartlett results; contracts do not prescribe pass/fail judgments.
`quality` defines process limits, complete control-chart rows, capability indices
and balanced crossed Gage R&R components. Control charts reuse `visualization::XyPlot`.
`doe` owns neutral factor ranges, level summaries, design-balance diagnostics, and coded
design specifications/results. Design rows contain a one-based run number and factor codes;
compact summaries are separate from full matrices. Response surface and dose-response
results reuse regression fits, adding stationary geometry or positive curve parameters.
`decision` owns weight/ranking methods, criterion direction and normalization
options, weight statistics, compact summaries and observation score rows. These
contracts have no node IDs or table handles; adapters decide how to expose tables.

`inference` defines interval rows, multiplicity-adjusted contrasts and cluster
inference reports. `regression::fit::FittedRegression` is the borrowed linear/binary
fit input shared by diagnostics and postestimation. `regression::postestimation`
owns evaluation settings and adjusted-mean results; binary marginal effects reuse
the same evaluation enum. No fitted designs or covariance matrices are copied by
the borrowed model contract.

- `execution`: `ScientificExecutionControl`, cancellation and computation errors.
  These types contain no execution-plan or project identities, and define no backend trait.
  `ScientificInputViolation` is the shared input-failure vocabulary for both
  `SciError` and `ScientificComputationError`; operation identity and execution
  interruption remain separate error concerns.
- `spatial`: coordinate weight rules and serializable `SpatialWeights<L>` with
  exact unit labels; global Moran inference; seven spatial regressions and balanced
  entity fixed-effect panels. Model reports distinguish conditional/reduced fits,
  innovations, point impacts and original observation/label indices. Covariance
  follows coefficient order and excludes the nuisance variance.
- `anova`: coded categorical factors, checked design dimensions, factorial and
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
  `prais` owns AR(1) transform and convergence settings; rho estimation uses
  lagged-residual regression. Linear results retain optional original WLS weights
  for downstream diagnostics.
  `models` owns predictor/category contracts, estimator/convergence
  options and structured coefficient/model/workflow results for the additional
  regression methods. Method-specific facts use `RegressionDetails`; undefined
  inference/metrics use optional fields. Generic category/group labels preserve
  original scalar identities, and workflows retain original predictor/row indices.
- `hypothesis`: neutral hypothesis requests, results, alternatives and errors.
- `longitudinal`: grouped-observation contracts for GEE and mixed models,
  response families, working correlation, ML/REML options, coefficient covariance,
  variance components, conditional random effects and ordered fitted/residual arrays.
  GEE has no likelihood/AIC; REML omits AIC. Fixed-only GLMM predictions explicitly
  set random effects to zero rather than integrating them out. Labels stay adapter-owned.
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
- `time_series::acf_pacf`: ACF/PACF requests and results, including the computed
  two-sided 95% white-noise reference-band half-width. `var` and `vec` own
  neutral fitted models and selected-summary options; `fit` shares multivariate
  equation/coefficient statistics, serial-test rows and stability roots.
  `forecast` owns ARIMA/smoothing/volatility options, source-aligned forecasts,
  ECM equations, Markov transition reports and PP/KPSS statistics. Forecast arrays
  start one step after the sample; unavailable initial fits and inference are nullable.
  KPSS explicitly distinguishes interpolated p-values from table-tail bounds.
- `diagnostics::serial_correlation`: serial-correlation requests and BG/Q/Durbin-Watson results.
- `diagnostics::model`: borrowed linear/binary model inputs, information criteria,
  nested-test results, influence summaries with separate observation arrays,
  collinearity/Harman reports and binary-outcome NRI/IDI point estimates.
  `survival::ProportionalHazardsResult` retains Cox time-interaction score tests.
- `diagnostics::residual`: residual/model test selection and BP/White/IM/RESET,
  normality, VIF and leverage results without numerical-backend types.
  VIF uses `None` for the intercept's undefined VIF/tolerance, preserving explicit
  optional values rather than encoding missing statistics as NaN.
- `panel`: estimator/effect options, `PanelFit`, selected-summary options and
  grouped coefficient/model/effect statistics. Estimator-specific statistics use
  an enum instead of unrelated optional fields on every result.
  `PanelData`, `DynamicPanelOptions`/`DynamicPanelFit` and `PanelTestOptions`/
  `PanelFisherTest` describe aligned lagged analyses, collapsed difference-GMM
  inference and individual/combined Fisher tests. Source rows describe differenced
  equation coordinates. An infinite Fisher statistic caused by a zero individual
  p-value uses `None` with combined p-value zero, never a nonfinite JSON number.
- `causal::iv`: IV estimator selection and fitted model facts, including design
  columns for later analyses. `InstrumentalVariableStatistics.model_test` owns
  the joint coefficient test as `InstrumentalVariableModelTest`: chi-square
  with its degrees by default, or F with numerator/denominator degrees for
  `small=true`. Both variants retain the statistic and p-value; serialization
  tags the current `modelTest` record by `distribution` with camel-case fields.
  `FirstStageResult` carries `betas`, the shared
  `RegressionCoefficientStatistics` as `inference`, actual `df_residual`, R² and
  adjusted R² alongside equation/variable names. The selected covariance inside
  that inference record supplies both the coefficient table and excluded-instrument
  F test. First-stage inference uses OLS residual degrees independently of the
  structural `small` option. First-stage, overidentification, Hausman and
  endogenous-regressor results are independent records, selected through
  `IvSummaryOptions` rather than embedded in every fit.
- `causal::did`: DID inputs, inference results and typed unavailable/error codes.
- `causal::models`: linear IV GMM, sharp RDD, treatment-group interaction tests,
  Heckman two-step, half-normal frontiers, SUR and synthetic-control options/results.
  `TreatmentResult` is the serializable neutral output of PSM/IPW/RA/AIPW, with
  distinct ATE/ATT records and explicitly nullable inference. Bootstrap options
  request iid row resampling with complete nuisance refitting. Group labels are
  generic; matching and prediction arrays retain row order, while retained RDD
  and selected Heckman rows and SUR predictor references are one-based.
  Randomization input contains observed columns, treatment/post indicators,
  repetitions and a reproducible seed; interruption remains an explicit failure.
- `density`: kernel-density input and output records.
- `visualization`: typed XY/reference lines, histograms, grouped distributions,
  intervals, ROC/AUC, category frequencies, rectangular matrices and coefficient
  intervals. Display bounds and sampling metadata contain no pixel layout,
  graph identity or result/window lifecycle.
- `descriptive`: empirical `GiniResult`, Dagum group/pair/component statistics. Group labels are generic so the node adapter can replace
  numerical group IDs with original scalar labels without changing statistics.
- `distribution`: probability distribution parameters and typed samples.
- `observation`: observation metadata, missing-value policy and category roles,
  exported from the crate root for all domains.
- `error`: stable operation and scientific error vocabulary.

- `meta`: independent study summaries and effect scales, fixed/DL/PM estimator and
  Wald/Knapp–Hartung inference options, model covariance and heterogeneity,
  omission records, combined P values and a funnel payload with an explicit
  descending standard-error domain. Study row details are separate from summary records.

`SciError` carries typed operation and failure facts. Runtime and Node Kernel
map those facts at their own boundaries; this crate does not maintain a separate
host-facing string-code mapping.
`SciError::into_computation_error` preserves the shared input violation
or computation failure when the caller's computation context already identifies
the operation. SCI composition and Runtime use this neutral conversion without
operation-name filtering or another error vocabulary.

`ScientificInputViolation` distinguishes missing/insufficient observations,
nonfinite observations, incompatible input shapes, data outside the statistic's
domain (`DataOutOfRange`, including nonidentifiable coefficients), and invalid
options (`ParameterOutOfRange`). Both scientific error types use this vocabulary;
data-domain failures must not be represented as invalid options.

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
`ClassicalTestResult::statistic` is an optional finite value. An unbounded Fisher
sample odds ratio is `None`/JSON null, while its exact p-value and table counts
remain available. Defined statistics retain their numeric JSON representation;
missing values are not replaced with a cross-product difference or nonfinite number.

Classical SCI and Runtime entrypoints return `ScientificComputationError`
directly. `ScientificInputViolation` distinguishes observation shape/domain,
nonfinite observations and invalid options; finite-input numerical breakdowns
are `ComputationFailed`. Cancellation and deadlines retain their shared variants.
Named linear-hypothesis parsing and inference retain `HypothesisError`, including
`InvalidInput(String)`, `Scientific(SciError)` and `Execution`.

Binary effect/classification records retain nullable inference for undefined rates;
regression fits retain named numeric designs for postestimation. Panel estimation
samples explicitly describe transformed coordinates and source-row groups. VAR
records retain selected lags, covariance divisor, exogenous names and sample rows.
These neutral records do not contain graph addresses or UI-specific types.

`path` owns observed-variable interaction/mediation options, conditional effects,
Johnson–Neyman regions and recursive equations/decompositions. Variable references are
zero-based indices; fits reuse the existing neutral regression result.

`survey` owns borrowed weight/stratum/PSU designs, explicit lonely-PSU handling, weight
summaries and design-inference records. Survey regression reuses the neutral regression
fit and keeps design degrees of freedom separate from observation count.

`power` owns prospective scalar design variants, request/alternative enums and results
with explicit sample units and total observations. It does not consume fitted model
state or infer effect sizes from observed significance.
