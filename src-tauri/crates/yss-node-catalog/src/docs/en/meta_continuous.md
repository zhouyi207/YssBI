# Continuous effect sizes

Connect treatment_mean, treatment_sd, treatment_n and reference_mean, reference_sd, reference_n from the same study table. Each row describes two independent groups; both sample sizes must be integers of at least 2. SDs can be zero individually, but must be nonnegative with positive resulting effect variance.
The default effect_measure is mean_difference: $y=\bar x_T-\bar x_C$, $v=s_T^2/n_T+s_C^2/n_C$. hedges_g uses $g=J(\nu)(\bar x_T-\bar x_C)/s_p$, pooled SD $s_p$, $\nu=n_T+n_C-2$ and exact gamma correction $J(\nu)=\Gamma(\nu/2)/[\sqrt{\nu/2}\Gamma((\nu-1)/2)]$. Its large-sample variance is $v=1/n_T+1/n_C+g^2/[2(n_T+n_C)]$.
Choose a common outcome and contrast direction. This does not estimate paired-group or change-score effects.

confidence_level defaults to 0.95 and must be strictly between 0 and 1. result reports measure, studies and confidence_level. studies is a reusable table with study (one-based input row), effect, variance, standard_error, lower and upper; limits are $y\pm z_{(1+c)/2}\sqrt v$ on the selected scale. These are study estimates, without a pooled significance test.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
