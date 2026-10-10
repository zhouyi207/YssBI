# Chi-square goodness-of-fit test

Compares observed category counts with specified expected counts.

## Inputs and parameters

`observed` and `expected` are numeric series in matching category order, of equal length with at least 2 entries. Observed counts must be nonnegative and expected counts strictly positive. All values must be finite and nonmissing. Database and in-memory series may be mixed and pair by current position, with equal lengths. There are no parameters.

Supply counts, not probabilities. The observed total must be positive and match the expected total, allowing floating-point rounding. The node does not normalize them or deduct degrees of freedom for parameters estimated from the data.

## Hypotheses and statistic

$H_0$: category probabilities follow the distribution specified by the expected counts. $H_1$: at least one category probability differs.

$$
X^2=\sum_{i=1}^{k}\frac{(O_i-E_i)^2}{E_i},\qquad
X^2\overset{H_0}{\approx}\chi^2_{k-1}.
$$

Here $k$ is the category count and $O_i,E_i$ are observed and expected frequencies. The reference distribution assumes independent observations and prespecified probabilities. Sparse categories affect approximation accuracy.

## Outputs and interpretation

`result` contains the structured result. `statistic` is $X^2$, `degrees_of_freedom` is fixed at `[k−1]`, `p_value` is the upper chi-square tail, and `sample_sizes` contains the observed total. `estimate` and `standard_error` are null.

$p<\alpha$ indicates departure from the specified distribution. If expectations were fitted to the same data, the unadjusted `k−1` degrees of freedom do not support that conclusion directly.
