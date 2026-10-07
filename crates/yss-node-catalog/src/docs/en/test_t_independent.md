# Independent-samples t test

Compares the means of two independent populations.

## Inputs and parameters

Connect numeric `group1` and `group2`, each with at least 2 finite observations. Sample sizes may differ; missing values must be handled beforehand. `equal_variance` defaults to `false` for Welch's test; `true` selects pooled variance. `alternative` defaults to `two_sided`, with `greater` and `less` available. Direction always means group 1 minus group 2.

## Hypotheses and statistic

$H_0:\mu_1-\mu_2=0$; the alternative is a nonzero, positive, or negative difference. Let $n_i,\bar x_i,s_i$ denote group size, mean, and sample standard deviation.

Welch's method uses $a=s_1^2/n_1$ and $b=s_2^2/n_2$:

$$
t=\frac{\bar x_1-\bar x_2}{\sqrt{a+b}},\qquad
\nu=\frac{(a+b)^2}{a^2/(n_1-1)+b^2/(n_2-1)}.
$$

The equal-variance method uses:

$$
s_p^2=\frac{(n_1-1)s_1^2+(n_2-1)s_2^2}{n_1+n_2-2},\qquad
SE=s_p\sqrt{1/n_1+1/n_2},\qquad \nu=n_1+n_2-2.
$$

Both use the $t_\nu$ reference distribution and require a positive standard error. Observations must be independent within and between groups. Pooled variance additionally assumes equal population variances.

## Outputs and interpretation

`result` contains the structured result: `statistic` is t, `estimate` is the group-1 minus group-2 mean, `standard_error` is the selected standard error, `degrees_of_freedom` is `[ν]`, and `sample_sizes` is `[n1, n2]`. Reject $H_0$ when `p_value` is below $\alpha$ for the selected alternative.
