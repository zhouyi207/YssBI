# Bayesian information criterion (BIC)

Connect a fitted OLS/WLS, Logit or Probit **model**. There are no parameters or refitting. GLS is unsupported; Gaussian residual sum of squares must be positive and binary fits must have converged.

$$
BIC=-2\ell+K\log n.
$$

$\ell$ is the maximized log-likelihood and $n$ is the fitted observation count. For OLS/WLS, $K$ counts regression coefficients, including the intercept, plus one error-variance parameter. WLS retains the precision-weight determinant term in its Gaussian likelihood. Logit/Probit reuse their saved Bernoulli likelihood and count coefficients only.

**result** contains criterion (bic), value, family, observations, parameters and log_likelihood. Including Gaussian scale increases BIC by $\log n$ relative to conventions excluding it. Parameter counting matches the AIC node.

Smaller BIC is preferred for models of the same response on the same observations under comparable likelihoods. This node neither tests nesting nor independently establishes the identity of samples from different fits, and has no p-value.
