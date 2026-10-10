# Survival decision curve

Connect aligned `time` and `event` series. Time must be finite and strictly positive, in one consistent unit; `event=1` (or true) means an observed event and `0` (or false) means right censoring. Rows with missing values are rejected, not dropped. Censoring is assumed independent of the event process, conditional on the model/group used. Inputs must have equal lengths and pair by current row position, including mixed database and in-memory series.

Connect `predicted_risk` in [0,1], for the same positive `survival_horizon=1` as the observed outcome. Parameters: `decision_threshold_min=0.01`, `decision_threshold_max=0.99`, with $0<\min<\max<1$; `decision_points=99` is an integer at least 2. Thresholds include both endpoints.

## Net benefit

At threshold $p$, select observations with predicted risk at least $p$. Let $q$ be their fraction and $r=1-\widehat S_{\rm selected}(t)$ their Kaplan–Meier event risk:
$$NB_{\rm model}=q\left[r-(1-r)\frac{p}{1-p}\right].$$
The model benefit is zero for an empty selected set. Treat-all uses $q=1$ and the whole-sample Kaplan–Meier risk; treat-none has benefit zero. Censoring is assumed independent within each selected set. This is an unstandardized benefit, without additional treatment/test harms.

`result` contains the model curve, dashed treat-all/treat-none reference lines, horizon, metadata, and `estimates` with threshold, selected count and all three benefits. Larger net benefit is preferable at a specified threshold under these assumptions. No uncertainty intervals or p-values are generated.

Nonempty subsets must support the chosen horizon unless their survival is already zero. Select `time`, `event`, `risk` from a model's `predictions` table to keep rows aligned. Fitting-sample results are apparent performance; no automatic cross-validation or probability recalibration is performed. See the [dcurves documentation](https://www.danieldsjoberg.com/dcurves/reference/dca.html).
