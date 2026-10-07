# One-proportion z test

Uses a normal approximation to test the population success probability of a binary event.

## Inputs and parameters

`series` is a nonempty numeric `0/1` sequence, where `1` means success. Missing values and other codes are rejected. `null_probability` defaults to `0.5` and must lie in $(0,1)$; endpoints give zero standard error. `alternative` defaults to `two_sided`, with `greater` and `less` available.

## Hypotheses and statistic

$H_0:p=p_0$, against $p\ne p_0$, $p>p_0$, or $p<p_0$. For $x$ successes in $n$ trials, let $\hat p=x/n$:

$$
z=\frac{\hat p-p_0}{\sqrt{p_0(1-p_0)/n}},\qquad z\overset{H_0}{\approx}N(0,1).
$$

Trials must be independent with a common success probability. Normal approximation needs adequate expected successes and failures; use an exact binomial test for small samples.

## Outputs and interpretation

`result` contains the structured result. `statistic` is z, `estimate` is $\hat p-p_0$, `standard_error` uses the null probability, `sample_sizes` is `[n]`, and degrees of freedom are empty. Reject the specified success probability when `p_value` is below $\alpha$.
