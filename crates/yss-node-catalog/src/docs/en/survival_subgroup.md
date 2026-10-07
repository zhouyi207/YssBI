# Stratified Cox subgroup analysis

Connect aligned `time` and `event` series. Time must be finite and strictly positive, in one consistent unit; `event=1` (or true) means an observed event and `0` (or false) means right censoring. Rows with missing values are rejected, not dropped. Censoring is assumed independent of the event process, conditional on the model/group used. Inputs must have equal lengths and pair by current row position, including mixed database and in-memory series.

Also connect binary `treatment`, a required `groups` label series and optional numeric adjustment `X₁, X₂, …`. At least two groups are required, with both treatment levels in each group. Original group labels are preserved. `survival_ties=efron` (default) or `breslow`; `max_iterations=500` (positive integer), `tolerance=0.0000001` (from 1e-12 to 0.01). Nonconvergence or a singular information matrix produces an error. Predictor columns must be finite and identifiable; categorical predictors require prior dummy coding.

## Model and heterogeneity test

Each group has its own baseline hazard and treatment slope, with shared adjustment slopes:
$$h_g(t\mid D,x)=h_{0g}(t)\exp(\beta_gD+x^\top\gamma).$$
$H_0:\beta_1=\cdots=\beta_G$ versus $H_1$: at least one treatment log hazard ratio differs. Let $C$ compare groups 2 through $G$ against group 1:

$$ W=(C\widehat\beta)^\top(C\widehat V C^\top)^{-1}(C\widehat\beta)
\ \overset{H_0}{\approx}\ \chi^2_{G-1}.$$
The contrast uses the joint covariance from one stratified fit, not a comparison of independently estimated p-values.

`result` contains group labels/counts/events, group treatment hazard ratios and 95% intervals, the equality test and the full stratified Cox model. Treatment columns precede adjustment columns. Individual coefficient tests use the standard normal Wald statistic under a zero-coefficient null. Group baselines are not compared by the equality test.

A small equality-test p-value indicates heterogeneity of the adjusted treatment association under the model. This is one executable analysis, not an automatic multi-node workflow expansion. No multiplicity adjustment, causal identification, automatic subgroup search or future prediction is performed.
$$
