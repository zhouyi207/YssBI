# Canonical correlation

Provide aligned numeric **X variable** and **Y variable** groups, at least one column each. Relational inputs must share a row domain. Missing values, constant columns and within-group rank deficiency are rejected. With $p,q$ variables, require $n>p+q$ observations. **Retained dimensions** defaults to 1, with $1\le k\le\min(p,q)$.

Variables are standardized using sample means and standard deviations. Whitening correlation blocks $R_{xx},R_{yy},R_{xy}$ gives canonical correlations $\rho_i$ and coefficients $a_i,b_i$:

$$u_i=Z_xa_i,\qquad v_i=Z_yb_i,\qquad \operatorname{Var}(u_i)=\operatorname{Var}(v_i)=1,\quad \operatorname{Corr}(u_i,v_i)=\rho_i.$$

Loadings correlate standardized original variables with their own group's scores; they differ from score coefficients.

The $i$th sequential test has $H_0$ setting this and all remaining canonical correlations to zero, versus at least one nonzero root. Bartlett's chi-square approximation to Wilks lambda is

$$\Lambda_i=\prod_{j=i}^{\min(p,q)}(1-\rho_j^2),\qquad \chi_i^2=-\left[n-1-\frac{p+q+1}{2}\right]\log\Lambda_i,\quad df_i=(p-i+1)(q-i+1).$$

Inference assumes independent observations and approximately joint multivariate normality. A root numerically indistinguishable from perfect correlation gives zero lambda and null chi-square/p-value rather than infinity.

**Result** contains all correlations and sequential tests, both groups' means/scales, and variable-by-retained-axis standardized weights/loadings. Tests use every root even when fewer score axes are retained.

**Scores** combines both groups in one independent computed table: `x_axis1`…`x_axisK`, followed by `y_axis1`…`y_axisK`, in input row order. Selected X/Y columns share a row domain for downstream correlation or plotting.

Reference: [R cancor](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/cancor.html).
