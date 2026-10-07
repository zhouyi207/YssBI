# Fixed-effect meta-analysis

Connect effects and strictly positive variances from the same study table. This model assumes a common true effect across independent studies. It fixes $\tau^2=0$ and pools $\hat\mu=\sum y_i/v_i\,/\,\sum1/v_i$, with conventional variance $1/\sum1/v_i$.
The reported heterogeneity diagnostics do not change the fixed-effect estimate; substantial heterogeneity calls for examining effect comparability and the model assumptions.

confidence_level defaults to 0.95, strictly between 0 and 1. inference defaults to wald: coefficient tests use $H_0:\beta_j=0$ versus $\beta_j\ne0$, with normal Wald inference. knapp_hartung multiplies covariance by $Q(\hat\tau^2)/(k-p)$ and uses $t_{k-p}$ for tests and intervals; $p=1$ for pooling. It can yield narrower intervals when the residual scale is below one. Variances are conditional on the fitted τ².

result contains studies, estimator, inference, coefficients (estimate, SE, statistic, P value, interval), covariance, residual_degrees_of_freedom, residual_q, heterogeneity and prediction_interval. heterogeneity reports fixed-weight Q and its chi-square P value for common effects against heterogeneity, with $df=k-p$, I², H² and τ². studies is a table with study, effect, variance, standard_error, lower, upper, weight (sums to one), fitted and residual. At least two studies and a full-rank design are required.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
