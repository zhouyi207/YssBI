# Score / LM test

Connect **restricted** and **full** fits for the same observations. Both must use the same OLS/WLS, Logit or Probit estimator; GLS is unsupported. There are no parameters or refitting. Row order, reconstructed responses, relative WLS weights and genuine design nesting are checked; $q=k_f-k_r>0$ and $n>k_f$ are required.

$H_0$ states that the additional coefficient restrictions hold. The score is evaluated at the restricted estimate:

$$
LM=U^\mathsf{T}I^{-1}U\ \mathrel{\dot\sim}\ \chi_q^2.
$$

$U$ is the full-model score at the restricted estimate and $I$ is expected information, including existing coefficients as nuisance parameters. Logit/Probit use Bernoulli probabilities and the appropriate link derivatives. For OLS/WLS this equals $n(1-SSR_f/SSR_r)$, with residual sums of squares under the same relative weights.

**result** contains both information-criterion summaries, restrictions and score (statistic, degrees_of_freedom, p_value). likelihood_ratio/f_test are null.

A small p-value supports additional nonzero coefficients. Inference requires regular likelihood assumptions. This node supplies neither heteroskedasticity-robust score inference nor specialized LM tests for other model families. Restricted binary probabilities must lie strictly between zero and one.
