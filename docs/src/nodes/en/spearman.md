# Spearman rank correlation

Connect aligned Numeric or Ordinal **X** and **Y**, with at least two complete observations. Ordinal columns use their explicitly declared level order; numeric columns use increasing values. Ties receive average ranks. Constant rankings are undefined and fail.

$$\rho_s=\operatorname{corr}(R_X,R_Y).$$

The null hypothesis is no rank association. **Alternative** defaults to `two_sided`, with `greater` and `less` available. **P-value method** defaults to `auto`: at most nine observations use exact permutations of Y's observation positions, including ties; larger samples use a t approximation. `permutation_exact` is limited to nine observations; `asymptotic` requires at least three.

Exact two-sided p-values are twice the smaller inclusive tail, capped at 1. Approximate inference uses $t=\rho_s\sqrt{(n-2)/(1-\rho_s^2)}$ with $n-2$ degrees of freedom; its accuracy improves with larger samples. No confidence interval is reported for rank correlations.

The single `result` records rho, sample size, the selected inference method, statistic, and p-value. Missing values are rejected and are not removed separately from each column.

References: [SciPy Spearman](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.spearmanr.html), [exact permutation tests](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.permutation_test.html).
