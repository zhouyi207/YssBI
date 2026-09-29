# Summary-statistics t test

Runs a t test from sample sizes, means, and sample standard deviations without raw observations.

## Inputs and parameters

The order of `series` depends on `design`:

| `design`                | Input order                           |
| ----------------------- | ------------------------------------- |
| `independent` (default) | `[n1, mean1, sd1, n2, mean2, sd2]`    |
| `one_sample`            | `[n, mean, sd]`                       |
| `paired`                | `[n, mean_difference, sd_difference]` |

Each $n$ must be an integer of at least 2. Means and standard deviations must be finite; standard deviations cannot be negative. Paired data require the standard deviation of the differences, not the two separate measurement standard deviations. Missing values are rejected.

`null_value` defaults to `0` and specifies the hypothesized mean or mean difference. `equal_variance` defaults to `false`; only independent samples use it to select Welch or pooled variance. `alternative` defaults to `two_sided`, with `greater` and `less` available.

## Hypotheses and statistic

$H_0:\theta=\theta_0$, where $\theta_0$ is `null_value`. Depending on design, $\theta$ is the population mean, paired mean difference, or group-1 minus group-2 mean difference. The alternative is unequal, greater, or less.

$$
t=\frac{\hat\theta-\theta_0}{SE},\qquad t\overset{H_0}{\sim}t_\nu.
$$

One-sample and paired designs use $SE=s/\sqrt n$ and $\nu=n-1$. Independent Welch inference uses $a=s_1^2/n_1$, $b=s_2^2/n_2$:

$$
SE=\sqrt{a+b},\qquad \nu=\frac{(a+b)^2}{a^2/(n_1-1)+b^2/(n_2-1)}.
$$

Pooled-variance inference uses:

$$
s_p^2=\frac{(n_1-1)s_1^2+(n_2-1)s_2^2}{n_1+n_2-2},\qquad
SE=s_p\sqrt{1/n_1+1/n_2},\qquad \nu=n_1+n_2-2.
$$

Standard error must be positive. Independence, pairing, and small-sample normality assumptions match the corresponding raw-data t test.

## Outputs and interpretation

`result` and `report` contain the same report. `method` identifies the design, `statistic` is t, `estimate` is $\hat\theta-\theta_0$, `standard_error` is $SE$, and `degrees_of_freedom` is `[ν]`. `sample_sizes` contains one or two sample sizes. Reject $H_0$ when `p_value` is below $\alpha$.
