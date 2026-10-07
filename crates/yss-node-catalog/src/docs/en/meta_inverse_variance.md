# Inverse-variance meta-analysis

Connect effects and strictly positive sampling variances from the same study table. The estimator setting defaults to fixed, with der_simonian_laird and paule_mandel available. The pooled effect is $\hat\mu=\sum w_i y_i/\sum w_i$, $w_i=1/(v_i+\tau^2)$; fixed sets $\tau^2=0$.
For DerSimonian–Laird, $\hat\tau^2=\max(0,(Q-k+1)/C)$ with $C=\sum1/v_i-\sum(1/v_i)^2/\sum1/v_i$. Paule–Mandel solves $Q(\tau^2)=k-1$, setting zero when $Q(0)\le k-1$.

confidence_level defaults to 0.95, strictly between 0 and 1. inference defaults to wald: coefficient tests use $H_0:\beta_j=0$ versus $\beta_j\ne0$, with normal Wald inference. knapp_hartung multiplies covariance by $Q(\hat\tau^2)/(k-p)$ and uses $t_{k-p}$ for tests and intervals; $p=1$ for pooling. It can yield narrower intervals when the residual scale is below one. Variances are conditional on the fitted τ².

result contains studies, estimator, inference, coefficients (estimate, SE, statistic, P value, interval), covariance, residual_degrees_of_freedom, residual_q, heterogeneity and prediction_interval. heterogeneity reports fixed-weight Q and its chi-square P value for common effects against heterogeneity, with $df=k-p$, I², H² and τ². studies is a table with study, effect, variance, standard_error, lower, upper, weight (sums to one), fitted and residual. At least two studies and a full-rank design are required.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
