# One-sample t test

Tests whether a population mean equals a specified value.

## Inputs and parameters

Connect a numeric `series` with at least 2 finite observations. Missing values are rejected. `null_mean` defaults to `0` and must be finite. `alternative` defaults to `two_sided`; `greater` and `less` are also available.

## Hypotheses and statistic

$H_0:\mu=\mu_0$, where $\mu_0$ is `null_mean`. The alternatives are $\mu\ne\mu_0$, $\mu>\mu_0$, or $\mu<\mu_0$.

$$
t=\frac{\bar x-\mu_0}{s/\sqrt n},\qquad t\overset{H_0}{\sim}t_{n-1}.
$$

Here $n$ is sample size, $\bar x$ the mean, and $s$ the sample standard deviation. Observations must be independent; small-sample inference assumes approximate normality. The standard error must be positive.

## Outputs and interpretation

`result` contains the structured result. `statistic` is t, `degrees_of_freedom` is `[n−1]`, and `p_value` uses the selected alternative. `estimate` is $\bar x-\mu_0$, `standard_error` is $s/\sqrt n$, and `sample_sizes` is `[n]`.

Reject the hypothesized mean when $p<\alpha$; otherwise there is insufficient evidence to reject it.
