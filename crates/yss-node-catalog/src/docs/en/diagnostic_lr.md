# Likelihood-ratio test

Connect the restricted fit to **restricted** and the larger fit to **full**. OLS/WLS, Logit and Probit are supported, with the same estimator on both sides. No parameters or refitting are used.

Upstream fits must use identical observations and row order. The node checks reconstructed response values row by row, WLS weights up to a common scale, and inclusion of the restricted design span in the full design. The full model must add at least one coefficient while retaining positive residual degrees of freedom. Binary fits must converge. GLS is unsupported.

$H_0$: the additional coefficient restrictions defining the smaller model hold. The alternative violates at least one restriction.

$$
LR=2(\ell_f-\ell_r)\ \mathrel{\dot\sim}\ \chi_q^2,\qquad q=k_f-k_r.
$$

Subscripts $f/r$ denote full/restricted fits and $k$ counts coefficients. Gaussian models use profile likelihood; binary models use Bernoulli likelihood. Regular inference assumes correctly specified, identifiable interior parameters. Linear models require independent normal errors with correctly specified variances/weights.

**result** includes restricted/full information-criterion summaries, restrictions and likelihood_ratio (statistic, degrees_of_freedom, p_value; denominator_df is null). Unrequested score/f_test are null. A small p-value supports the additional terms, not necessarily better prediction.
