# Runs test for randomness

Tests whether the order of a binary sequence is consistent with random arrangement.

## Inputs and parameters

`series` contains numeric `0/1` observations in the intended analysis order, with at least 2 observations and both values present. There are no parameters. The node does not sort the sequence or dichotomize continuous values at their median. Missing values are rejected.

## Hypotheses and statistic

$H_0$: sequence order is random conditional on category counts. $H_1$: there are too many or too few runs. A run is a consecutive stretch of the same value. Let $R$ count runs, $n_0,n_1$ count the two values, and $n=n_0+n_1$:

$$
E(R)=1+\frac{2n_0n_1}{n},\qquad
V(R)=\frac{2n_0n_1(2n_0n_1-n)}{n^2(n-1)},
$$

$$
Z=\frac{R-E(R)}{\sqrt{V(R)}}\overset{H_0}{\approx}N(0,1).
$$

Variance must be positive. The node uses a two-sided normal approximation without continuity correction or an exact method. Define what order means before analysis; small or highly imbalanced category counts can weaken the approximation.

## Outputs and interpretation

`result` and `report` are identical. `statistic_name` is `runs_z` and `statistic` is $Z$, not the raw run count. `p_value` is two-sided and `sample_sizes` is `[n]`. Degrees of freedom are empty; estimate and standard error are null.

$p<\alpha$ indicates nonrandom ordering. A nonsignificant result does not prove independence.
