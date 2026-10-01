# Moran's I

Measure global spatial association between an attribute and neighboring attributes.

## Inputs and settings

`weights` is a spatial design; `units` must uniquely match its complete unit set; `response` is finite and nonconstant. Columns must align and missing values are rejected. Exact identifiers align the weights; islands retain zero rows. At least two observations are required. `spatial_permutations` defaults to 999, is nonnegative, and 0 disables permutation inference. `seed` is nonnegative, default 42. Identical inputs, order and seed reproduce results.

## Statistic and hypotheses

$$
I=\frac{n}{S_0}\frac{z^TWz}{z^Tz},\qquad z_i=y_i-\bar y,\qquad S_0=\sum_{ij}w_{ij},\qquad E(I)=-\frac1{n-1}.
$$

$H_0$: no association between the attribute assignment and location. $H_1$: positive or negative spatial association. Values above the null expectation indicate clustering of similar values; lower values indicate dissimilar neighbors. $I$ is not necessarily bounded by $[-1,1]$.

Under normality, let $S_1=\frac12\sum_{ij}(w_{ij}+w_{ji})^2$ and $S_2=\sum_i(\sum_jw_{ij}+\sum_jw_{ji})^2$:

$$
V_N(I)=\frac{n^2S_1-nS_2+3S_0^2}{(n^2-1)S_0^2}-E(I)^2.
$$

Randomization holds attribute values fixed and permutes their positions. For $n>3$, with $b_2=n\sum_i z_i^4/(\sum_i z_i^2)^2$:

$$
V_R(I)=\frac{n[(n^2-3n+3)S_1-nS_2+3S_0^2]-b_2[(n^2-n)S_1-2nS_2+6S_0^2]}{(n-1)(n-2)(n-3)S_0^2}-E(I)^2.
$$

Both analytical tests use standard-normal $z=(I-E(I))/\sqrt{V(I)}$ approximations and two-sided p-values. Insufficient sample size or degenerate variance yields null inference. Variance at or below $10^{-14}$ is treated as numerically degenerate.

Permutations hold $W$ fixed and shuffle values over all units. Two-sided extremeness uses $|I-E(I)|$. If $b$ of $B$ draws are at least as extreme, $p=(b+1)/(B+1)$. This differs from reporting the smaller single-tail permutation probability and has no parametric degrees of freedom.

## Output

`statistic` and `expected` report $I$ and its null expectation. `normal_variance`, `normal_z`, `normal_p_value` and the corresponding `randomization_*` fields report analytical inference. `permutation_p_value` is null when disabled. `observations`, `permutations` and `seed` record settings. Degenerate fully connected weights can yield null analytical inference and a permutation p-value of 1.

This node analyzes raw attributes globally. It does not provide regression-residual-adjusted inference or local LISA tests.

[Moran assumptions and inference](https://pysal.org/esda/stable/user-guide/global_morans_i.html)
