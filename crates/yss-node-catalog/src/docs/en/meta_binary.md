# Binary effect sizes

Connect treatment_events, treatment_n, reference_events and reference_n. Counts must be nonnegative integers with events no greater than the corresponding positive total. Let $a,b,c,d$ be treatment events/non-events and reference events/non-events; $n_T=a+b$, $n_C=c+d$.
effect_measure defaults to log_odds_ratio: $y=\log(ad/bc)$, $v=1/a+1/b+1/c+1/d$. log_risk_ratio uses $y=\log[(a/n_T)/(c/n_C)]$, $v=1/a-1/n_T+1/c-1/n_C$. risk_difference uses $y=a/n_T-c/n_C$, $v=ab/n_T^3+cd/n_C^3$.
continuity_correction defaults to 0.5 and must be nonnegative. If any cell is zero, it is added to all four cells of that study before calculation. Setting 0 disables correction; nonfinite effects or zero variances cannot be pooled. Log ratios remain on the log scale.

confidence_level defaults to 0.95 and must be strictly between 0 and 1. result reports measure, studies and confidence_level. studies is a reusable table with study (one-based input row), effect, variance, standard_error, lower and upper; limits are $y\pm z_{(1+c)/2}\sqrt v$ on the selected scale. These are study estimates, without a pooled significance test.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
