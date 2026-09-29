# Mann–Kendall trend test

Tests for monotonic trend in ordered observations.

## Inputs and parameters

`series` needs at least 3 finite numeric values with no missing entries. Its current order is the analysis order. The node accepts no time column and does not sort or seasonally adjust observations. `alternative` defaults to `two_sided`; `greater` tests increasing trend and `less` decreasing trend.

## Hypotheses and statistic

$H_0$: there is no monotonic trend. $H_1$: trend exists in the selected direction. For $n$ observations:

$$
S=\sum_{i<j}\operatorname{sgn}(x_j-x_i),\qquad
Z=\begin{cases}
(S-1)/\sqrt V,&S>0,\\
0,&S=0,\\
(S+1)/\sqrt V,&S<0.
\end{cases}
$$

Without ties, $V=n(n-1)(2n+5)/18$ and $Z\overset{H_0}{\approx}N(0,1)$. The current tie adjustment subtracts $\sum_g(t_g^3-t_g)/18$, where $t_g$ are tied-group sizes. This differs from the [usual Mann–Kendall tie correction](https://search.r-project.org/CRAN/refmans/trend/html/mk.test.html); results with ties need separate verification.

Observations must be independent. The node does not correct serial correlation or seasonality. Positive $Z$ indicates increasing trend and negative $Z$ decreasing trend.

## Outputs and interpretation

`result` and `report` are identical. `statistic_name` is `S_corrected_z`, `statistic` is $Z$, and `details.s_statistic` is $S$. `details.kendall_tau` is $S/\binom n2$, not tie-adjusted tau-b. `p_value` uses the selected direction and `sample_sizes` is `[n]`. Degrees of freedom are empty; estimate and standard error are null.

When the reference assumptions hold, $p<\alpha$ supports monotonic trend. No trend slope is estimated.
