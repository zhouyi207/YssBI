# Mann–Whitney U test

Compares distributions or locations of two independent populations.

## Inputs and parameters

`group1` and `group2` must be nonempty finite numeric series without missing values. Each group is read independently; lengths may differ, and database and in-memory inputs may be mixed. The statistical design still treats them as independent samples, not paired differences. `alternative` defaults to `two_sided`, with `greater` and `less` referring to group 1 relative to group 2.

## Hypotheses and statistic

$H_0$: both populations have the same distribution. The two-sided alternative is a distribution difference; directional alternatives place group 1 stochastically above or below group 2. A location interpretation requires comparable distribution shapes.

Rank the pooled data using average ranks for ties. Let $R_1$ be group 1's rank sum, $n_1,n_2$ the group sizes, $N=n_1+n_2$, and $t_j$ the tied-group sizes:

$$
U_1=R_1-\frac{n_1(n_1+1)}2,\qquad
V=\frac{n_1n_2}{12}\left[N+1-\frac{\sum_j(t_j^3-t_j)}{N(N-1)}\right],
$$

$$
Z=\frac{U_1-n_1n_2/2}{\sqrt V}\overset{H_0}{\approx}N(0,1).
$$

The node uses a tie-corrected normal approximation with no exact method or continuity correction. All-identical data cannot be tested. Small-sample approximation needs care.

## Outputs and interpretation

`result` contains the structured result. `statistic` is group 1's $U_1$, `details.u_complement` is $n_1n_2-U_1$, `p_value` uses the selected tail of $Z$, and `sample_sizes` contains both group sizes. Degrees of freedom are empty; estimate and standard error are null.

Reject equal distributions when $p<\alpha$. Do not interpret this unconditionally as a difference in means.
