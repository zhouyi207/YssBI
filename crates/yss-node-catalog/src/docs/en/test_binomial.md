# Exact binomial test

Tests the success probability of binary trials using binomial probabilities.

## Inputs and parameters

Supply numeric `0/1` observations through `series`, with `1` indicating success. Between 1 and 1,000,000 trials are supported. Missing values and other codes are rejected. `null_probability` defaults to `0.5` and lies in $[0,1]$. `alternative` defaults to `two_sided`; `greater` and `less` are available.

## Hypotheses and statistic

$H_0:p=p_0$; alternatives are $p\ne p_0$, $p>p_0$, or $p<p_0$. The statistic is the observed number of successes $x$, with reference model:

$$
X\overset{H_0}{\sim}\operatorname{Binomial}(n,p_0),\qquad
P(X=k)=\binom nk p_0^k(1-p_0)^{n-k}.
$$

The two-sided p-value sums outcomes whose probabilities do not exceed that of the observation. One-sided alternatives use the corresponding binomial tail. Trials must be independent with a common success probability.

## Outputs and interpretation

`result` contains the structured result. `statistic_name` is `successes`; `statistic` and `details.successes` are the success count. `estimate` is $x/n$, and `sample_sizes` is `[n]`. `standard_error` is null and degrees of freedom are empty. Reject $H_0$ when `p_value` is below $\alpha$.
