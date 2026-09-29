# Chi-square test of independence

Builds a contingency table from paired categorical observations and tests independence.

## Inputs and parameters

`row` and `column` describe the same observations, with Numeric, Categorical, Ordinal, or Binary semantics. They must be nonempty, aligned, and equal in length, with at least 2 observed levels in each variable. Relational series must share a row domain. Missing values are rejected; categories are not merged or dropped automatically. There are no parameters.

## Hypotheses and statistic

$H_0$: row and column variables are independent. $H_1$: they are associated.

$$
E_{ij}=\frac{O_{i+}O_{+j}}N,\qquad
X^2=\sum_{i=1}^{r}\sum_{j=1}^{c}\frac{(O_{ij}-E_{ij})^2}{E_{ij}},
\qquad X^2\overset{H_0}{\approx}\chi^2_{(r-1)(c-1)}.
$$

$O_{ij}$ is an observed count, $O_{i+},O_{+j}$ are marginal totals, and $N$ is total sample size. Pearson's statistic is used without Yates correction. Independent observations and adequate expected counts are needed for the chi-square approximation.

## Outputs and interpretation

`result` and `report` are identical: `statistic` is $X^2$, `degrees_of_freedom` is `[(r−1)(c−1)]`, `p_value` is the upper chi-square tail, and `sample_sizes` is `[N]`. `details.minimum_expected_count` gives the smallest expected count. `estimate` and `standard_error` are null.

$p<\alpha$ provides evidence of association, not causation. Fisher's exact test is an alternative for sparse $2\times2$ tables.
