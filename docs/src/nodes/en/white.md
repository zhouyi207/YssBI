# White heteroskedasticity test

Connect an OLS or WLS `model` to test variance patterns in predictors, their squares and cross-products. There are no parameters. GLS is unsupported; the original model should include an intercept and at least one predictor.

## Hypotheses and formula

- **Null $H_0$:** homoskedasticity; all nonconstant squared-residual auxiliary coefficients are zero.
- **Alternative $H_1$:** at least one is nonzero, indicating variance related to these terms.

Regress squared residuals on $Z$, containing original predictors, their squares and pairwise products:

$$
LM=nR_{\mathrm{aux}}^2,\qquad
q=\operatorname{rank}(Z)-1,\qquad
LM\overset{H_0}{\approx}\chi_q^2.
$$

$n$ is fitted sample size and $R_{\mathrm{aux}}^2$ the centered auxiliary coefficient of determination. Degrees of freedom use effective rank, without counting redundant terms twice.

WLS replaces the dependent quantity with $w_i u_i^2$, retaining an unweighted auxiliary regression on original $Z$. Here $w_i$ are original precision weights and $u_i$ residuals. Sample size must exceed the full expanded column count.

## Outputs and interpretation

`result` contains the structured result: `test=white` with inner `result` fields `lm_stat`, `df` and `p_value`. P-values use the chi-square upper tail; $p<\alpha$ rejects homoskedasticity. Non-rejection means insufficient evidence.

Method details: [White test](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.het_white.html).
