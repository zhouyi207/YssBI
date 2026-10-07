# Paired t test

Tests whether the mean difference between two measurements on the same subjects is zero.

## Inputs and parameters

Connect numeric `before` and `after` series in matching subject order. Lengths must agree, with at least 2 finite pairs. Handle missing values jointly upstream; the node does not match subjects or remove incomplete pairs. `alternative` defaults to `two_sided`; `greater` and `less` are also available.

## Hypotheses and statistic

Define $d_i=before_i-after_i$. $H_0:\mu_d=0$; alternatives are $\mu_d\ne0$, $\mu_d>0$, or $\mu_d<0$.

$$
t=\frac{\bar d}{s_d/\sqrt n},\qquad t\overset{H_0}{\sim}t_{n-1}.
$$

Here $n$ is the number of pairs and $\bar d,s_d$ are the mean and sample standard deviation of differences. Subjects must be independent. Small-sample inference assumes approximately normal differences; their standard deviation must be positive.

## Outputs and interpretation

`result` contains the structured result. `statistic` is t, `estimate` is the mean `before−after` difference, `standard_error` is $s_d/\sqrt n$, `degrees_of_freedom` is `[n−1]`, and `sample_sizes` is `[n]`.

`greater` tests whether the before mean exceeds the after mean; `less` tests the reverse. Reject zero mean difference when `p_value` is below $\alpha$.
