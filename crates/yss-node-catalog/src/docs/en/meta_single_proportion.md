# Single-proportion effect sizes

Connect events and sample_size. Both are integer counts, $0\le x\le n$ and $n\ge1$. effect_measure defaults to logit_proportion: $y=\log[p/(1-p)]$, $v=1/x+1/(n-x)$. proportion uses $y=p=x/n$, $v=p(1-p)/n$. arcsine_proportion uses $y=\arcsin\sqrt p$, $v=1/(4n)$.
continuity_correction defaults to 0.5 and must be nonnegative. For raw and logit proportions only, a boundary study ($x=0$ or $n$) uses $x+c,n+2c$; arcsine uses original counts. Correction 0 can produce unusable zero/infinite variance at boundaries. Intervals are normal intervals on the selected analysis scale and are not clipped; transform logit or arcsine results before interpreting them as rates.

confidence_level defaults to 0.95 and must be strictly between 0 and 1. result reports measure, studies and confidence_level. studies is a reusable table with study (one-based input row), effect, variance, standard_error, lower and upper; limits are $y\pm z_{(1+c)/2}\sqrt v$ on the selected scale. These are study estimates, without a pooled significance test.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
