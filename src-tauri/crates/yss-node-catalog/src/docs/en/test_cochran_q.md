# Cochran Q test

Compares binary success probabilities across three or more conditions measured on the same subjects.

## Inputs and parameters

Add 3–16 numeric `0/1` series under `conditions`, with at least 2 subjects. Columns must be equal in length, aligned by subject, and use a consistent meaning for `1`. Relational series must share a row domain. Missing values and other codes are rejected. There are no parameters.

## Hypotheses and statistic

$H_0$: all conditions have equal marginal success probabilities. $H_1$: at least one differs. Let $k$ be condition count, $C_j$ successes in condition $j$, $R_i$ successful conditions for subject $i$, and $T=\sum_j C_j$:

$$
Q=\frac{(k-1)\left(k\sum_j C_j^2-T^2\right)}
{kT-\sum_i R_i^2}
\overset{H_0}{\approx}\chi^2_{k-1}.
$$

The denominator must be positive, requiring within-subject variation across conditions. Subjects must be independent; repeated measurements within subjects remain paired. The current chi-square approximation needs care with small samples.

## Outputs and interpretation

`result` and `report` are identical. `statistic` is $Q$, `degrees_of_freedom` is `[k−1]`, `p_value` is the upper chi-square tail, and `sample_sizes` contains subjects per condition. Estimate and standard error are null.

Reject equal condition probabilities when $p<\alpha$. The node does not identify which condition pairs differ.
