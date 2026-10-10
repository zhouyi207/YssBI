# Nested-model comparison

Connect **restricted** and **full** fits using the same OLS/WLS, Logit or Probit estimator. There are no parameters or refitting. The full model must add coefficients and retain $n>k_f$. Reconstructed responses, relative WLS weights and restricted-design column-space inclusion are checked. Upstream fits must use the same subjects and row order. GLS is unsupported.

$H_0$ states that the smaller model's additional coefficient restrictions hold; the alternative violates at least one. With $q=k_f-k_r$:

$$
LR=2(\ell_f-\ell_r)\ \mathrel{\dot\sim}\ \chi_q^2,\qquad
LM=U^\mathsf{T}I^{-1}U\ \mathrel{\dot\sim}\ \chi_q^2.
$$

The score uses expected information at the restricted estimate. OLS/WLS also provide

$$
F=\frac{(SSR_r-SSR_f)/q}{SSR_f/(n-k_f)}\sim F_{q,n-k_f}.
$$

$SSR$ uses the same relative precision weights. The F reference requires independent normal errors with correct weights; LR/score use regular asymptotic likelihood inference.

**result** contains both models' family, observations, parameters, log_likelihood, aic and bic, plus restrictions, likelihood_ratio, score and f_test. For binary models f_test is null. Gaussian information criteria include one error-variance parameter; test degrees of freedom use only the coefficient-count difference. Nonnested models are rejected.
