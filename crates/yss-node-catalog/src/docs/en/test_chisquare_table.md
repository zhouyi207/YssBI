# Pearson chi-square contingency test

Tests independence using an existing contingency table of counts.

## Inputs and parameters

`counts` is a row-major sequence of nonnegative integer frequencies. `rows` and `columns` both default to `2` and accept integers from `2–1000`. Input length must equal their product.

For `rows=2` and `columns=3`, the order is `[row1_col1, row1_col2, row1_col3, row2_col1, row2_col2, row2_col3]`. Values must be finite with no missing entries; every row and column total must be positive.

## Hypotheses and statistic

$H_0$: row and column classifications are independent. $H_1$: they are associated.

$$
E_{ij}=\frac{O_{i+}O_{+j}}N,\qquad
X^2=\sum_{ij}\frac{(O_{ij}-E_{ij})^2}{E_{ij}},
\qquad X^2\overset{H_0}{\approx}\chi^2_{(r-1)(c-1)}.
$$

$O_{ij}$ is an input count, $N$ the total count, and $r,c$ the table dimensions. Pearson's statistic has no continuity correction. Counts should come from independent observations. Small expected counts can make the approximation unreliable.

## Outputs and interpretation

`result` contains the structured result. `statistic` is $X^2$, `degrees_of_freedom` is `[(r−1)(c−1)]`, `sample_sizes` is `[N]`, and `details.minimum_expected_count` is the minimum expected count. `estimate` and `standard_error` are null.

`p_value` is the upper chi-square tail. Reject independence when $p<\alpha$.
