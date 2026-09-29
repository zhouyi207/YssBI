# Poisson rate test

Tests the mean event rate for observations with equal exposure.

## Inputs and parameters

`series` contains nonnegative integer counts and must be nonempty with no missing values. Observations should have equal exposure. `null_rate` defaults to `1`: the finite, nonnegative expected count per observation under the null. `alternative` defaults to `two_sided`, with `greater` and `less` available.

Total observed events are limited to 1,000,000 and total null expected events to 500,000. Unequal exposure is not an input to this node.

## Hypotheses and statistic

$H_0:\lambda=\lambda_0$; alternatives are $\lambda\ne\lambda_0$, $\lambda>\lambda_0$, or $\lambda<\lambda_0$. Let $S=\sum_i x_i$ and $m=n\lambda_0$:

$$
S\overset{H_0}{\sim}\operatorname{Poisson}(m),\qquad
P(S=k)=e^{-m}\frac{m^k}{k!}.
$$

The two-sided p-value sums Poisson probability masses no greater than the observed mass; one-sided alternatives use the selected direction. Observations must be independent and follow a Poisson count model. Overdispersion affects inference.

## Outputs and interpretation

`result` and `report` contain the same report. `estimate` is $S/n$, `standard_error` is $\sqrt{S/n}/\sqrt n$, and `sample_sizes` is `[n]`. `details.event_count` and `details.expected_event_count` are $S$ and $m$.

`statistic_name` is `count_score`, with value $(S-m)/\max(\sqrt m,1)$. `p_value` comes from Poisson probabilities, not a normal approximation to this score. Degrees of freedom are empty. Reject the specified rate when $p<\alpha$.
