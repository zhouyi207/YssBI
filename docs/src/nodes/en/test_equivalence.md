# One-sample equivalence test (TOST)

Tests whether a population mean lies inside a prespecified equivalence interval.

## Inputs and parameters

`series` needs at least 2 finite observations, no missing values, and positive sample standard deviation. `lower_bound` defaults to `-0.5` and `upper_bound` to `0.5`. Both must be finite, with lower strictly below upper. Bounds are absolute mean limits in the units of the data. There is no `alternative` parameter.

## Hypotheses and statistics

Let the bounds be $L,U$. $H_0:\mu\le L$ or $\mu\ge U$; $H_1:L<\mu<U$. With $SE=s/\sqrt n$:

$$
t_L=\frac{\bar x-L}{SE},\qquad t_U=\frac{\bar x-U}{SE},\qquad \nu=n-1.
$$

The lower test compares $H_{0L}:\mu\le L$ against $H_{1L}:\mu>L$; the upper compares $H_{0U}:\mu\ge U$ against $H_{1U}:\mu<U$:

$$
p_L=P(t_\nu\ge t_L),\qquad p_U=P(t_\nu\le t_U),\qquad
p_{\mathrm{TOST}}=\max(p_L,p_U).
$$

Here $n,\bar x,s$ are sample size, mean, and sample standard deviation. Observations must be independent and approximately normal for small-sample inference. Specify bounds before inspecting results.

## Outputs and interpretation

`result` contains the structured result. `statistic` is $t_L$ and `details.t_upper_bound` is $t_U$. `details.p_lower_bound` and `details.p_upper_bound` contain the one-sided p-values. `p_value` is their maximum, `estimate` is the mean, `standard_error` is $SE$, degrees of freedom are `[n−1]`, and sample size is `[n]`.

Equivalence is supported only when both one-sided tests reject at level $\alpha$. A nonsignificant ordinary difference test is not evidence of equivalence.
