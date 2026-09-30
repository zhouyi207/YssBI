# Bartlett test

Tests equality of variances across independent groups under approximate population normality.

## Inputs and parameters

Add 2–16 numeric series under `groups`, each with at least 2 finite observations and strictly positive sample variance. The current node requires equal lengths; relational series must share a row domain. Missing values are rejected. There are no parameters.

## Hypotheses and statistic

$H_0:\sigma_1^2=\cdots=\sigma_k^2$. $H_1$: at least one variance differs. Let $n_i,s_i^2$ denote group sizes and sample variances, with $N=\sum_i n_i$:

$$
s_p^2=\frac{\sum_i(n_i-1)s_i^2}{N-k},\qquad
C=1+\frac{\sum_i1/(n_i-1)-1/(N-k)}{3(k-1)},
$$

$$
B=\frac{(N-k)\ln s_p^2-\sum_i(n_i-1)\ln s_i^2}{C}
\overset{H_0}{\approx}\chi^2_{k-1}.
$$

Observations and groups must be independent. The test is sensitive to non-normality: skewness, heavy tails, or outliers can cause rejection without variance differences being the sole explanation.

## Outputs and interpretation

`result` contains the structured result. `statistic_name` is `chi_squared`, `statistic` is $B$, `degrees_of_freedom` is `[k−1]`, `p_value` is the upper chi-square tail, and `sample_sizes` contains group sizes. Estimate and standard error are null.

When normality is plausible, $p<\alpha$ supports unequal variances. Consider Levene or Brown–Forsythe tests for clearly non-normal data.
