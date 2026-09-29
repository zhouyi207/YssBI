# White heteroskedasticity test

Connect an OLS or WLS `model` to examine variance patterns in predictors, their squares and cross-products. There are no parameters. The fitted sample is reused; GLS is unsupported.

## Hypotheses and calculation

**Null $H_0$:** homoskedastic errors; all nonconstant terms in the auxiliary equation below have zero coefficients.

**Alternative $H_1$:** at least one coefficient is nonzero, indicating variance related to these terms.

For design $X=[\mathbf1,x_1,\ldots,x_p]$, residuals $u_i$ and sample size $n$, construct

$$
Z=[X,x_1^2,\ldots,x_p^2,x_1x_2,\ldots,x_{p-1}x_p],
\qquad v_i=u_i^2.
$$

Regress $v$ on $Z$ by OLS, obtaining $\hat v$. Then

$$
R_{\mathrm{aux}}^2
=1-\frac{\sum_i(v_i-\hat v_i)^2}{\sum_i(v_i-\bar v)^2},
\qquad LM=nR_{\mathrm{aux}}^2,\quad q=\operatorname{rank}(Z)-1,
$$

$$
LM\overset{H_0}{\approx}\chi_q^2,\qquad p=1-F_{\chi_q^2}(LM).
$$

Degrees of freedom use effective rank: identities such as $D^2=D$ for a dummy do not count twice. See the [statsmodels White test](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.het_white.html).

## WLS and applicability

WLS uses $v_i=w_i u_i^2$, retaining the fit's positive precision weights, but regresses $v$ on the original $Z$ **without auxiliary weights**. It then uses the same $nR^2$. This examines patterns in weighted squared errors; interpret the null in relation to the model's variance weights.

The current expansion assumes the first design column is an intercept; use a model with an intercept. At least one nonconstant regressor is required, and $n$ must exceed the full expanded column count $1+2p+p(p-1)/2$, before rank reduction. Redundant columns are handled through truncated SVD. Nonfinite computations fail; invariant squared residuals are assigned $R^2=0$.

Normal errors are not required, but independence, a correctly specified conditional mean and suitable moment conditions matter. Robust covariance in the source model does not make this test cluster- or serial-correlation robust.

## Outputs and interpretation

`result` and `report` are identical: `test=white` with inner `result` fields `lm_stat`, `df` and `p_value`.

Reject at a preselected level $\alpha$ if $p<\alpha$; otherwise evidence is insufficient. For one regressor $x$, the auxiliary design is $1,x,x^2$, generally giving two degrees of freedom. If $x$ is binary, $x^2=x$ and the effective degrees of freedom become one.

The expansion can detect more variance patterns than fitted-value BP, but uses more terms and may lose power. Rejection can also reflect misspecification of the mean equation; non-rejection does not establish homoskedasticity.
