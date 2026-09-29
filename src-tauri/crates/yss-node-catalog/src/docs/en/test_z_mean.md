# One-sample z test for a mean

Tests a population mean when the population standard deviation is known.

## Inputs and parameters

`series` must contain at least 1 finite numeric observation and no missing values. `null_mean` defaults to `0` and must be finite. `population_sd` defaults to `1` and must be a known, positive population standard deviation. `alternative` defaults to `two_sided`; `greater` and `less` are available.

## Hypotheses and statistic

$H_0:\mu=\mu_0$; alternatives are $\mu\ne\mu_0$, $\mu>\mu_0$, or $\mu<\mu_0$.

$$
z=\frac{\bar x-\mu_0}{\sigma/\sqrt n},\qquad z\overset{H_0}{\sim}N(0,1).
$$

Here $n$ is sample size and $\sigma$ is `population_sd`. Observations must be independent. The reference distribution is exact for a normal population and otherwise relies on a large-sample approximation. Use a t test when population SD is unknown rather than treating sample SD as known $\sigma$.

## Outputs and interpretation

`result` and `report` are identical. `statistic` is z, `estimate` is $\bar x-\mu_0$, `standard_error` is $\sigma/\sqrt n$, `sample_sizes` is `[n]`, and `degrees_of_freedom` is empty. `p_value` uses the selected direction; reject $H_0$ when $p<\alpha$.
