# Akaike information criterion (AIC)

Connect a fitted OLS/WLS, Logit or Probit **model**. There are no parameters or refitting. GLS is unsupported. Gaussian residual sum of squares must be positive and binary fits must have converged.

$$
AIC=-2\ell+2K.
$$

Here $\ell$ is the maximized log-likelihood and $K$ counts estimated parameters. OLS/WLS use the Gaussian profile likelihood, including the precision-weight log determinant for WLS. Their $K$ counts regression coefficients, including the intercept, plus one error-variance parameter. Logit/Probit reuse the retained Bernoulli likelihood and count regression coefficients only.

**result** contains criterion (aic), value, family, observations, parameters and log_likelihood. Gaussian scale is included; relative to software defaults excluding scale, AIC is larger by 2. [Parameter-count convention](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.RegressionResults.info_criteria.html)

Smaller values are preferred for the same response, sample and likelihood convention. AIC is a relative criterion without a p-value; obtaining two numbers does not establish model comparability.
