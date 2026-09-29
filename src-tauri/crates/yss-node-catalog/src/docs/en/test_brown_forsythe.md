# Brown–Forsythe test

Compares variances of independent populations using group medians as centers. This is the median-centered Levene variant, not a robust ANOVA of means.

## Inputs and parameters

Add 2–16 numeric series under `groups`, each with at least 2 finite observations and no missing values. The current node requires equal lengths; relational series must share a row domain. There are no parameters.

## Hypotheses and statistic

$H_0:\sigma_1^2=\cdots=\sigma_k^2$. $H_1$: at least one population variance differs.

Let $c_i$ be group $i$'s median and define absolute deviations $z_{ij}=|x_{ij}-c_i|$.

For group sizes $n_i$, $N=\sum_i n_i$, group deviation means $\bar z_i$, and overall deviation mean $\bar z$:

$$
F=\frac{\sum_i n_i(\bar z_i-\bar z)^2/(k-1)}
{\sum_i\sum_j(z_{ij}-\bar z_i)^2/(N-k)}
\overset{H_0}{\approx}F_{k-1,N-k}.
$$

Within-group deviation variation must be positive. Samples must be independent.

Median centering reduces the influence of skewness and extreme values on the center, but small-sample inference remains approximate.

## Outputs and interpretation

`result` and `report` are identical. `statistic` is $F$, `degrees_of_freedom` lists numerator and denominator degrees as `[k−1, N−k]`, `p_value` is the upper F tail, and `sample_sizes` contains group sizes. Estimate and standard error are null.

Reject equal variances when $p<\alpha$. A nonsignificant result is insufficient evidence against equality, not proof of identical variances.
