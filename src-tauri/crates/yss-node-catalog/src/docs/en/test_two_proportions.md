# Two-proportion z test

Compares success probabilities in two independent populations.

## Inputs and parameters

Connect nonempty numeric `0/1` series to `group1` and `group2`. `1` means success; sample sizes may differ. Missing values and other codes are rejected. `null_difference` defaults to `0` and specifies the hypothesized group-1 minus group-2 proportion difference. It must be finite and meaningful as a probability difference. `alternative` defaults to `two_sided`, with `greater` and `less` available.

## Hypotheses and statistic

$H_0:p_1-p_2=\delta_0$, against a difference unequal to, greater than, or less than $\delta_0$. Let $x_i,n_i,\hat p_i=x_i/n_i$ be success counts, trial counts, and sample proportions:

$$
z=\frac{\hat p_1-\hat p_2-\delta_0}{SE},\qquad z\overset{H_0}{\approx}N(0,1).
$$

For $\delta_0=0$, use the pooled proportion $\hat p=(x_1+x_2)/(n_1+n_2)$:

$$
SE=\sqrt{\hat p(1-\hat p)(1/n_1+1/n_2)}.
$$

For a nonzero difference, use $SE=\sqrt{\hat p_1(1-\hat p_1)/n_1+\hat p_2(1-\hat p_2)/n_2}$. Standard error must be positive. Groups and observations must be independent, with adequate successes and failures for normal approximation.

## Outputs and interpretation

`result` and `report` are identical. `statistic` is z, `estimate` is $\hat p_1-\hat p_2-\delta_0$, `standard_error` is the selected standard error, `sample_sizes` is `[n1, n2]`, and degrees of freedom are empty. Interpret `p_value` in the selected direction; reject $H_0$ when $p<\alpha$.
