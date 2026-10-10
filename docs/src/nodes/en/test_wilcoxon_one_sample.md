# One-sample Wilcoxon signed-rank test

Tests whether a sample distribution has a specified center of symmetry.

## Inputs and parameters

`series` accepts finite numeric observations with no missing values. `null_median` defaults to `0` and must be finite. Differences are $d_i=x_i-m_0$, where $m_0$ is `null_median`. Zero differences are excluded from ranking, leaving at least 2 nonzero differences.

`alternative` defaults to `two_sided`, with `greater` and `less` available.

## Hypotheses and statistic

$H_0$: the population is symmetric about $m_0$. Alternatives place its center away from, above, or below $m_0$. Observations must be independent. This is not a test of median equality that is valid without a symmetry assumption.

Rank the nonzero $|d_i|$, assigning average ranks $r_i$ to ties. Let $W^+$ be the positive rank sum:

$$
W^+=\sum_{d_i>0}r_i,\qquad
Z=\frac{W^+-\frac12\sum_i r_i}{\sqrt{\frac14\sum_i r_i^2}}.
$$

For $n\le20$ nonzero differences, p-values use exact sign enumeration. Larger samples use a $N(0,1)$ approximation without continuity correction. Two-sided inference compares departures from the rank-sum center; one-sided inference uses the selected direction.

## Outputs and interpretation

`result` contains the structured result. `statistic_name` is `signed_rank`, but `statistic` contains standardized $Z$. The raw $W^+$ is `details.positive_rank_sum`. `details.exact_p_value_used` is `1` for exact enumeration and `0` for normal approximation. `sample_sizes` is `[n]` after excluding zero differences. Degrees of freedom are empty; estimate and standard error are null.

Reject the specified symmetric center when `p_value` is below $\alpha$. Many zero differences reduce the effective sample size.
