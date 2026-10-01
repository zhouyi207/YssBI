# Weibull survival regression

Connect aligned `time` and `event` series. Time must be finite and strictly positive, in one consistent unit; `event=1` (or true) means an observed event and `0` (or false) means right censoring. Rows with missing values are rejected, not dropped. Censoring is assumed independent of the event process, conditional on the model/group used. Equal-length columns from unrelated tables are not treated as aligned.

Zero or more numeric `predictors` are allowed; an intercept is always fitted. $S(t\mid x)=\exp\{-\exp[(\log t-\eta)/\sigma]\}$; Weibull shape is $1/\sigma$.
`survival_horizon=1` is a strictly positive prediction time. `max_iterations=500` (positive integer), `tolerance=0.0000001` (from 1e-12 to 0.01). Nonconvergence or a singular information matrix produces an error. Predictor columns must be finite and identifiable; categorical predictors require prior dummy coding.

## Model and inference

Let $\eta=\beta_0+x^\top\beta$ and $\log T=\eta+\sigma\varepsilon$. Maximum likelihood incorporates censoring:
$$\ell=\sum_i\{\delta_i\log f(t_i\mid x_i)+(1-\delta_i)\log S(t_i\mid x_i)\}.$$
The scale is estimated jointly with regression coefficients except for exponential models. The full observed information, including coefficient–scale dependence, supplies covariance. For coefficient $j$, $H_0:\beta_j=0$ versus $H_1:\beta_j\ne0$ uses $z=\widehat\beta_j/SE$ with a standard normal reference and two-sided p-value. The 95% interval is $\widehat\beta_j\pm1.96SE$.

`result` contains coefficients, exponentiated coefficients (`time_ratios`), covariance, scale and its standard error, log likelihood, AIC, BIC, iterations, counts, horizon, row-level median survival and event probabilities. Covariance order is raw regression coefficients followed by log(scale) when scale is estimated; exponential scale standard error is null. Exponentiated slopes multiply survival times, not hazards; the exponentiated intercept is the baseline time scale. Weibull/exponential median is $\exp(\eta)(\log2)^\sigma$; the other medians are $\exp(\eta)$.

`predictions` is a DataFrame with `time`, `event`, and `risk` in original input order. Select all three columns from this output to connect calibration or decision-curve nodes; use the same `survival_horizon` throughout. These are predictions for the fitting rows, so evaluating them is apparent, in-sample performance.
At least one event and a full-rank design with more rows than regression coefficients are required. There is no delayed entry, interval censoring, frailty or weighting. Parametric prediction beyond observed follow-up is allowed and relies on the chosen distribution.
