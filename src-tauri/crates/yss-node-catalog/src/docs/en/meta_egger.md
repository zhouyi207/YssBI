# Egger asymmetry test

Connect effects and positive variances for at least three independent studies with varying SEs. Regress $y_i/s_i=\alpha+\beta(1/s_i)+e_i$, $s_i=\sqrt{v_i}$, by OLS with an intercept. The design must be full rank.
The asymmetry test is $H_0:\alpha=0$ versus $H_1:\alpha\ne0$; the intercept t statistic uses residual variance and $k-2$ degrees of freedom. confidence_level defaults to 0.95, strictly between 0 and 1.
result is the standardized-response regression summary: coefficients named asymmetry_intercept and precision, covariance, residual degrees of freedom and confidence intervals. Use the intercept P value for Egger's test. Asymmetry can reflect heterogeneity or other small-study effects and does not establish publication bias; few studies give unreliable inference.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
