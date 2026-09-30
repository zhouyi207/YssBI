# Ramsey RESET specification test

Connect an OLS or WLS `model` to test whether nonlinear powers improve its mean equation. Fitted observations and WLS weights are reused; GLS is unsupported.

| Parameter             | Augmentation                                                                                     |
| --------------------- | ------------------------------------------------------------------------------------------------ |
| `rhs=false` (default) | Normalize fitted values to 0–1 and add their second, third and fourth powers; $q=3$              |
| `rhs=true`            | Normalize each nonconstant, non-0/1 predictor to 0–1 and add its second, third and fourth powers |

## Hypotheses and formula

- **Null $H_0$:** all selected additional coefficients are zero.
- **Alternative $H_1$:** at least one additional coefficient is nonzero.

$$
F=\frac{(\mathrm{RSS}_R-\mathrm{RSS}_U)/q}
{\mathrm{RSS}_U/(n-k-q)},\qquad
F\overset{H_0}{\sim}F_{q,n-k-q}.
$$

$\mathrm{RSS}_R$ and $\mathrm{RSS}_U$ are the original and augmented residual sums of squares. $n$ is sample size, $k$ the original column count and $q$ the number of added terms. WLS uses original weights in both sums.

The node requires $n>k+q$ and full rank after augmentation. RHS mode excludes constants and 0/1 predictors, adds no interactions and fails if no eligible predictors remain.

## Outputs and interpretation

`result` contains the structured result: `test=reset` with inner `result` fields `f_stat`, `df1=q`, `df2=n-k-q` and `p_value`.

The p-value uses the F upper tail. $p<\alpha$ indicates joint significance of the selected powers and motivates checking the mean equation, without identifying a specific variable to add. Upstream robust covariance does not switch this node to robust RESET.

Method details: [Ramsey RESET](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.linear_reset.html).
