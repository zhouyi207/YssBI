# Paired Wilcoxon signed-rank test

Tests whether paired differences are symmetrically distributed around zero.

## Inputs and parameters

Connect numeric `before` and `after` series in matching subject order. Lengths must agree; relational series must share a row domain. Differences are $d_i=before_i-after_i$. Missing and non-finite values are rejected. Zero differences are excluded from ranking, leaving at least 2 nonzero differences.

`alternative` defaults to `two_sided`, with `greater` and `less` available. Direction follows `before−after`.

## Hypotheses and statistic

$H_0$: the difference distribution is symmetric about zero. Alternatives place its center away from, above, or below zero. Subjects must be independent; a location interpretation requires symmetric differences.

Rank the nonzero $|d_i|$, assigning average ranks $r_i$ to ties. Let $W^+$ be the positive rank sum:

$$
W^+=\sum_{d_i>0}r_i,\qquad
Z=\frac{W^+-\frac12\sum_i r_i}{\sqrt{\frac14\sum_i r_i^2}}.
$$

For $n\le20$ nonzero differences, p-values use exact sign enumeration. Larger samples use a $N(0,1)$ approximation without continuity correction. Two-sided inference compares departures from the rank-sum center; one-sided inference uses the selected direction.

## Outputs and interpretation

`result` contains the structured result. `statistic_name` is `signed_rank`, but `statistic` contains standardized $Z$. The raw $W^+$ is `details.positive_rank_sum`. `details.exact_p_value_used` is `1` for exact enumeration and `0` for normal approximation. `sample_sizes` is `[n]` after excluding zero differences. Degrees of freedom are empty; estimate and standard error are null.

Reject the specified symmetric center when `p_value` is below $\alpha$. Many zero differences reduce the effective sample size.
