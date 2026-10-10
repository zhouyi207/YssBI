# Meta-regression

Connect aligned effects, positive variances and one or more numeric moderators. An intercept is always included; the design must have full rank and $k>p$, where $p$ includes the intercept. Coefficients use the original moderator units and input column names.
The model is $y=X\beta+u+\epsilon$, fitted by $\hat\beta=(X'WX)^{-1}X'Wy$, $W_{ii}=1/(v_i+\tau^2)$. estimator defaults to paule_mandel; fixed sets zero heterogeneity. Paule–Mandel solves residual $Q(\tau^2)=k-p$; der_simonian_laird uses $\max(0,(Q(0)-k+p)/\operatorname{tr}P)$, $P=W-WX(X'WX)^{-1}X'W$ at $\tau^2=0$.
Heterogeneity fields use residual $df=k-p$ and assess unexplained study variation. Study-level associations do not establish individual-level or causal effects.

confidence*level defaults to 0.95, strictly between 0 and 1. inference defaults to wald: coefficient tests use $H_0:\beta_j=0$ versus $\beta_j\ne0$, with normal Wald inference. knapp_hartung multiplies covariance by $Q(\hat\tau^2)/(k-p)$ and uses $t*{k-p}$ for tests and intervals; $p=1$ for pooling. It can yield narrower intervals when the residual scale is below one. Variances are conditional on the fitted τ².

result contains studies, estimator, inference, coefficients (estimate, SE, statistic, P value, interval), covariance, residual_degrees_of_freedom, residual_q, heterogeneity and prediction_interval. heterogeneity reports fixed-weight Q and its chi-square P value for common effects against heterogeneity, with $df=k-p$, I², H² and τ². studies is a table with study, effect, variance, standard_error, lower, upper, weight (sums to one), fitted and residual. At least two studies and a full-rank design are required.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
