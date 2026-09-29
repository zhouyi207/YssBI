# VAR Granger causality tests

Connect a VAR Fit `model` to assess whether another variable's history adds linear predictive information for an equation, conditional on the fitted information set. There are no parameters; lag selection, coefficients and covariance come from the existing fit.

## Individual-variable tests

Write the $K$-variable VAR as

$$
y_t=c+\sum_{\ell\in\mathcal L}A_\ell y_{t-\ell}+Bz_t+u_t,
$$

where $\mathcal L$ is the actual included lag set, $m=|\mathcal L|$, and $z_t$ represents any other included terms. For response equation $i$ and another variable $j$:

**Null $H_0$:** $(A_\ell)_{ij}=0$ for every $\ell\in\mathcal L$; $j$ does not Granger-cause $i$ conditional on the included information.

**Alternative $H_1$:** at least one of these lag coefficients is nonzero.

Let $b=R\hat\beta_i$ select the included lag coefficients for $j$, and let $V_i$ be the equation's coefficient covariance:

$$
W=b'(RV_iR')^{-1}b,\qquad
W\overset{H_0}{\approx}\chi_m^2,\qquad
p=1-F_{\chi_m^2}(W).
$$

A model containing lags $\{1,3\}$ has two restrictions here, not three.

## ALL joint test

**Null $H_0$:** all included lags of every other variable have zero coefficients in equation $i$.

**Alternative $H_1$:** at least one of those coefficients is nonzero.

Select all lags for all $j\ne i$ with $R$ and use the same Wald formula, with $(K-1)m$ degrees of freedom. This is a joint test within one response equation, not a sum of individual p-values. See the [statsmodels VAR causality discussion](https://www.statsmodels.org/stable/vector_ar.html#granger-causality).

## Outputs and interpretation

`result` and `report` are identical. Each row of `vargranger` contains `eq_name` (response), `excluded` (tested predictor or `ALL`), `chi2`, `df` and `p_value`. A univariate VAR has no other variable to exclude and returns an empty array.

For example, `eq_name=consumption`, `excluded=income` and $p<0.05$ at a preselected 5% level reject the claim that past income adds no predictive information. This is not the reverse direction or an intervention-based causal effect.

Inference needs an appropriate VAR specification, sample size, error assumptions and treatment of stationarity. Ordinary VAR Wald reference distributions can be invalid for untreated unit-root/cointegrated series. Invalid fits or noninvertible covariance blocks fail. Rows receive no multiplicity correction, and non-rejection only indicates insufficient evidence for this information set and lag specification.
