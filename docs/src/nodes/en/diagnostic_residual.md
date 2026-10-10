# Residual diagnostics

Connect an OLS or WLS **model**. The fitted sample, design and original precision weights are reused without refitting. There are no parameters. GLS is unsupported; the design must have full column rank and $n>k$, including the intercept in $k$.

Raw residuals are $e_i=y_i-\hat y_i$ and weighted residuals are $u_i=\sqrt{w_i}e_i$ (OLS uses $w_i=1$). Let $s^2=\sum_i u_i^2/(n-k)$ and let $h_i$ be the weighted hat-matrix diagonal. Internally standardized residuals are

$$
r_i=\frac{u_i}{s\sqrt{1-h_i}}.
$$

Externally studentized residuals replace $s^2$ by the deleted-observation variance $s_{(-i)}^2=[\sum u^2-u_i^2/(1-h_i)]/(n-k-1)$. Cook's distance is $r_i^2h_i/[k(1-h_i)]$.

**result** contains raw-residual mean, sample standard deviation and range, weighted residual sum of squares, residual standard error, maximum leverage and maximum Cook's distance.

**observations** is a pageable, connectable table with observation (one-based), fitted, residual, weighted_residual, leverage, standardized_residual, studentized_residual and cooks_distance. Rows retain fitted-sample order. Zero residual variance, unit leverage or insufficient deleted-case degrees of freedom/variance produce null where a diagnostic is undefined. Normality and heteroskedasticity tests are separate analyses.
