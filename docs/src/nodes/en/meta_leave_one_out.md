# Leave-one-study-out analysis

Connect effects and positive variances for at least three independent studies. Each study is omitted once and the pooled model, including τ², is refitted. estimator defaults to paule_mandel; fixed and der_simonian_laird are also available.
result is the full-data baseline model. Compare its pooled effect and heterogeneity with the studies table to identify influential studies; omission is a sensitivity analysis, not an automatic deletion rule.

confidence*level defaults to 0.95, strictly between 0 and 1. inference defaults to wald: coefficient tests use $H_0:\beta_j=0$ versus $\beta_j\ne0$, with normal Wald inference. knapp_hartung multiplies covariance by $Q(\hat\tau^2)/(k-p)$ and uses $t*{k-p}$ for tests and intervals; $p=1$ for pooling. It can yield narrower intervals when the residual scale is below one. Variances are conditional on the fitted τ².

studies contains omitted_study (one-based input row), estimate, standard_error, lower, upper, tau_squared and i_squared_percent for each refit. The baseline reports coefficient tests of zero effect and Cochran Q homogeneity; omission rows report intervals and heterogeneity magnitudes, without an additional influence P value.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
