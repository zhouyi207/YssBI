# Friedman repeated-measures test

Compares relative locations across three or more conditions measured on the same subjects.

## Inputs and parameters

Add 3–16 numeric series under `conditions`. They must be equal in length and aligned in the same subject order, with at least 2 subjects. Relational series must share a row domain. Values must be finite and nonmissing; handle incomplete subjects jointly upstream. There are no parameters.

## Hypotheses and statistic

$H_0$: all repeated conditions have the same location. $H_1$: at least one differs. Rank the $k$ condition values separately within each subject, using average ranks for ties. For $n$ subjects and condition rank sums $R_j$:

$$
Q_0=\frac{12}{nk(k+1)}\sum_{j=1}^{k}R_j^2-3n(k+1),\qquad
C=1-\frac{\sum_{i,g}(t_{ig}^3-t_{ig})}{nk(k^2-1)},
$$

$$
Q=\frac{Q_0}{C}\overset{H_0}{\approx}\chi^2_{k-1}.
$$

$t_{ig}$ is the size of tied group $g$ within subject $i$. $C$ must be positive. Subjects must be independent. The node uses asymptotic chi-square inference, not exact permutation inference.

## Outputs and interpretation

`result` and `report` are identical. `statistic` is corrected $Q$, `degrees_of_freedom` is `[k−1]`, `p_value` is the upper chi-square tail, and `sample_sizes` is `[n, …, n]` for the conditions. Estimate and standard error are null.

$p<\alpha$ supports a condition difference. Pairwise comparisons are not produced automatically.
