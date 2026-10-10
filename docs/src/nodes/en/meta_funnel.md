# Funnel plot

Connect effects and positive variances for at least two independent studies. estimator defaults to paule*mandel, with fixed and der_simonian_laird alternatives. confidence_level defaults to 0.95 and must be strictly between 0 and 1.
The plot places effect $y_i$ against standard error $s_i=\sqrt{v_i}$, with zero SE at the top. Reference boundaries are $\hat\mu\pm z*{(1+c)/2}s$ around the pooled estimate, with a center line. These are sampling-error contours, not a confidence interval for τ² or a formal publication-bias test.
result contains data, xLabel, yLabel, referenceLines and metadata. Pooling uses all studies; at most 2048 study points are displayed, with sampling declared in metadata. Asymmetry needs study-context interpretation.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
