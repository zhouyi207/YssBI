# Pearson correlation

Connect aligned Numeric series to **X** and **Y**, with at least three finite paired observations. Missing values are rejected; filter the common sample upstream. Both variables must vary.

$$r=\frac{\sum_i(x_i-\bar x)(y_i-\bar y)}{\sqrt{\sum_i(x_i-\bar x)^2\sum_i(y_i-\bar y)^2}}.$$

**Alternative** defaults to `two_sided`; `greater` tests positive correlation and `less` negative correlation. Under $H_0:\rho=0$, $t=r\sqrt{(n-2)/(1-r^2)}$ has $n-2$ degrees of freedom for independent bivariate-normal observations.

**Confidence level** defaults to `0.95` and must lie strictly between 0 and 1. The two-sided Fisher interval uses $\operatorname{atanh}(r)\pm z_{1-\alpha/2}/\sqrt{n-3}$, then transforms back with tanh. It is null for three observations. Alternative affects the test, not the interval.

The single structured `result` includes `coefficient`, `observations`, `alternative`, `inference`, and `confidence_interval`. Perfect correlation can have a null, infinite-limit test statistic with a finite limiting p-value. Correlation describes linear association and does not establish agreement between measurement methods.

Reference: [SciPy Pearson correlation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.pearsonr.html).
