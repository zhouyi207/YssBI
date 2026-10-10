# Cook's distance

Connect an OLS/WLS **model**. Its fitted residuals, design and precision weights measure the influence of deleting each observation. The source model is not refitted and there are no parameters. The design must have full column rank with $n>k$; GLS is unsupported.

Let $u_i=\sqrt{w_i}(y_i-\hat y_i)$, with $w_i=1$ for OLS; $s^2=\sum u_i^2/(n-k)$, $h_i$ is weighted leverage and $k$ counts regression coefficients including the intercept:

$$
D_i=\frac{u_i^2}{k s^2}\frac{h_i}{(1-h_i)^2}.
$$

**result** includes sample size, coefficient count, residual summaries, maximum leverage and maximum_cooks_distance. **observations** provides observation (one-based), fitted, residual, weighted_residual, leverage, standardized_residual, studentized_residual and cooks_distance for each fitted row. The table supports paging, column selection and downstream plotting.

Undefined diagnostics, including those with zero residual variance or unit leverage, are null. Larger distances indicate stronger influence, not an outlier significance test. The node supplies no p-value or automatic deletion threshold. WLS diagnostics describe the weighted regression geometry.
