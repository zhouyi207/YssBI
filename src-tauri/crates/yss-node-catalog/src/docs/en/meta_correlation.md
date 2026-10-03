# Correlation effect sizes

Connect correlation and sample_size from the same study table. Pearson correlations must satisfy $|r|<1$ and sample sizes must be integers greater than 3. The effect uses Fisher's transform $y=\operatorname{atanh}(r)$ with variance $v=1/(n-3)$. Pool these transformed effects; use $\tanh$ on the pooled estimate and interval limits to recover the correlation scale. Inputs are independent unadjusted correlations, not partial correlations with unspecified covariates.

confidence_level defaults to 0.95 and must be strictly between 0 and 1. result reports measure, studies and confidence_level. studies is a reusable table with study (one-based input row), effect, variance, standard_error, lower and upper; limits are $y\pm z_{(1+c)/2}\sqrt v$ on the selected scale. These are study estimates, without a pooled significance test.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
