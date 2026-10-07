# Cox proportional hazards regression

Connect aligned `time` and `event` series. Time must be finite and strictly positive, in one consistent unit; `event=1` (or true) means an observed event and `0` (or false) means right censoring. Rows with missing values are rejected, not dropped. Censoring is assumed independent of the event process, conditional on the model/group used. Inputs must have equal lengths and pair by current row position, including mixed database and in-memory series.

Connect one or more numeric `X₁, X₂, …`; no intercept column is added. `survival_ties=efron` selects Efron (default) or `breslow` partial likelihood for tied events. `survival_horizon=1` must be positive and no later than maximum observed follow-up. `max_iterations=500` (positive integer), `tolerance=0.0000001` (from 1e-12 to 0.01). Nonconvergence or a singular information matrix produces an error. Predictor columns must be finite and identifiable; categorical predictors require prior dummy coding.

## Model and inference

$$ h(t\mid x)=h_0(t)\exp(x^\top\beta),\qquad
\widehat S(t\mid x)=\exp\{-\widehat H_{0,\bar x}(t)\exp[(x-\bar x)^\top\widehat\beta]\}.$$
At every event time, the risk set includes subjects observed at that time. The reported baseline is the **Breslow** cumulative baseline at the predictor means, even when coefficients use Efron ties, and includes the event at the listed time. Before the first event, the predicted event probability is zero.

`result` is a typed Cox model: coefficients, covariance, hazard ratios with 95% intervals, partial log likelihood, iterations, sample/event counts, predictor means/ranges, centered linear predictors and baseline points. For each coefficient, $H_0:\beta_j=0$ versus $H_1:\beta_j\ne0$ uses $z=\widehat\beta_j/SE$ and a standard normal reference; intervals exponentiate to hazard-ratio intervals. A hazard ratio above 1 indicates a higher instantaneous hazard at fixed covariates.

`predictions` is a DataFrame with `time`, `event`, and `risk` in original input order. Select all three columns from this output to connect calibration or decision-curve nodes; use the same `survival_horizon` throughout. These are predictions for the fitting rows, so evaluating them is apparent, in-sample performance.
Connect `result` to the Cox nomogram. Proportional hazards, independent subjects and conditionally independent censoring are assumptions, not automatically tested. No penalization, robust/cluster covariance, weights or stratification is exposed here; use the dedicated time-dependent or subgroup node for those supported designs.
$$
