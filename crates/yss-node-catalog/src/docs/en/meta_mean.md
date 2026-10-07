# Mean effect sizes

Connect mean, sd and sample_size for each independent study. The study effect is its mean, $y=\bar x$, with sampling variance $v=s^2/n$. SD must be positive and sample size an integer of at least 2. The output is a one-group mean, rather than a difference between groups. All studies must measure the same quantity in compatible units.

confidence_level defaults to 0.95 and must be strictly between 0 and 1. result reports measure, studies and confidence_level. studies is a reusable table with study (one-based input row), effect, variance, standard_error, lower and upper; limits are $y\pm z_{(1+c)/2}\sqrt v$ on the selected scale. These are study estimates, without a pooled significance test.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
