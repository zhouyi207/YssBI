# Levene test

Compares variances of independent populations using group means as centers.

## Inputs and parameters

Add at least two numeric series under `groups`, each with at least 2 finite observations and no missing values. The current node requires equal lengths; relational series must share a row domain. There are no parameters.

## Hypotheses and statistic

$H_0:\sigma_1^2=\cdots=\sigma_k^2$. $H_1$: at least one population variance differs.

Let $c_i=\bar x_i$ be group $i$'s mean and define absolute deviations $z_{ij}=|x_{ij}-c_i|$.

For group sizes $n_i$, $N=\sum_i n_i$, group deviation means $\bar z_i$, and overall deviation mean $\bar z$:

$$
F=\frac{\sum_i n_i(\bar z_i-\bar z)^2/(k-1)}
{\sum_i\sum_j(z_{ij}-\bar z_i)^2/(N-k)}
\overset{H_0}{\approx}F_{k-1,N-k}.
$$

Within-group deviation variation must be positive. Samples must be independent.

The method is more robust to non-normality than Bartlett's test, although mean centering is affected by extreme values. Use the Brown–Forsythe node for median centering.

## Outputs and interpretation

`result` contains the structured result. `statistic` is $F$, `degrees_of_freedom` lists numerator and denominator degrees as `[k−1, N−k]`, `p_value` is the upper F tail, and `sample_sizes` contains group sizes. Estimate and standard error are null.

Reject equal variances when $p<\alpha$. A nonsignificant result is insufficient evidence against equality, not proof of identical variances.
