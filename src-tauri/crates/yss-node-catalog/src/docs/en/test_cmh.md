# Cochran–Mantel–Haenszel test

Tests a common association between binary exposure and outcome while controlling for strata.

## Inputs and parameters

`exposed` and `outcome` use `0/1` codes or Boolean binary values. `strata` identifies each subject's stratum with Numeric, Categorical, Ordinal, or Binary semantics. All three inputs must be nonempty, aligned, equal in length, and nonmissing. Relational series must share a row domain. There are no parameters.

## Hypotheses and statistic

$H_0$: exposure and outcome are conditionally independent within strata. $H_1$: a common-direction within-stratum association exists. Write stratum $h$ as $\begin{pmatrix}a_h&b_h\\c_h&d_h\end{pmatrix}$, with $a_h$ counting exposure and outcome both equal to 1. Let $n_h$ be its total, $r_h=a_h+b_h$, and $c_h^{+}=a_h+c_h$:

$$
E_h=\frac{r_h c_h^{+}}{n_h},\qquad
V_h=\frac{r_h(n_h-r_h)c_h^{+}(n_h-c_h^{+})}{n_h^2(n_h-1)},
$$

$$
X^2_{\mathrm{CMH}}=\frac{[\sum_h(a_h-E_h)]^2}{\sum_h V_h}
\overset{H_0}{\approx}\chi^2_1.
$$

No continuity correction is used. Single-subject strata provide no information; total variance must be positive. Observations must be independent, with comparable association directions across strata. This is not a test of effect heterogeneity.

## Outputs and interpretation

`result` and `report` are identical. `statistic` is CMH chi-square, `degrees_of_freedom` is `[1]`, `p_value` is the upper tail, and `sample_sizes` contains total observations. `estimate` and `standard_error` are null; no common odds-ratio estimate is returned.

$p<\alpha$ supports association after stratification; it does not automatically rule out other confounders.
