# Cox nomogram

Connect the typed `result` output of the static Cox node to `model`. Parameters: `survival_horizon=1`, positive and within model follow-up; `nomogram_ticks=5`, an integer at least 2. At least one baseline event at or before the horizon and a nonzero predictor contribution range are required. Time-dependent and subgroup reports are not compatible model inputs.

## Points and survival

For predictor $j$, let $a_j$ be the minimum centered log-hazard contribution over its observed range, and $\Delta_j$ its contribution span. Use $c=100/\max_j\Delta_j$:

$$
P_j=c{\beta_j(x_j-\bar x_j)-a_j},\qquad
S(t\mid P)=\exp[-H_{0,\bar x}(t)\exp\{\sum_j a_j+P/c\}],
$$

where $P=\sum_jP_j$. Thus the predictor with the largest observed effect span covers 100 points. A negative slope reverses that predictor's value scale.

Read a predictor value vertically against the top Points axis, sum points across predictors, and locate the total on Total points to read survival probability below. This displays **survival**, not event probability. The mapping uses the fitted model's observed predictor ranges and centered Breslow baseline; it does not refit the model.

`result` contains axis labels and precomputed normalized tick positions/labels, horizon, maximum total points, points per log hazard and baseline cumulative hazard. The renderer only lays out these values. There are no additional p-values or confidence intervals. The chart inherits Cox assumptions and the fitting range; extrapolation beyond that range is not represented.
