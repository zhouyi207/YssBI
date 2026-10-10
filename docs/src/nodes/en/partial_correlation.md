# Partial correlation

Connect aligned Numeric **X**, **Y**, and one or more **Control variable** inputs. Add controls in Details. All columns must share the same sample. If there are $q$ controls, require $n>q+2$ observations.

Regress X and Y on an intercept and the controls, then compute Pearson correlation $r_{XY\cdot C}$ between their residuals. Constant or linearly dependent controls, or zero residual variation, fail explicitly; columns are not dropped automatically.

Under $H_0:\rho_{XY\cdot C}=0$, the statistic is

$$t=r_{XY\cdot C}\sqrt{\frac{n-q-2}{1-r_{XY\cdot C}^2}},\quad df=n-q-2.$$

**Alternative** is `two_sided` by default, with `greater` and `less` available. **Confidence level** defaults to `0.95`. The approximate two-sided Fisher interval uses standard error $1/\sqrt{n-q-3}$ and is null when $n\leq q+3$. Inference assumes independent observations and an appropriate linear, normally distributed residual model.

`result` includes `coefficient`, `control_variables`, sample size, inference, and the confidence interval. This is Pearson partial correlation, not partial rank correlation.

Reference: [R correlation inference](https://search.r-project.org/R/refmans/stats/html/cor.test.html).
