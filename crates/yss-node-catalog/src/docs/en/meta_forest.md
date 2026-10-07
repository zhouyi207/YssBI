# Forest plot

Connect effects and positive variances for at least two independent studies. estimator defaults to paule_mandel, with fixed and der_simonian_laird alternatives. confidence_level defaults to 0.95 (strictly between 0 and 1); inference defaults to wald, with knapp_hartung available for the pooled interval.
Study intervals are $y_i\pm z\sqrt{v_i}$. The Pooled row uses the selected model's coefficient interval. exponentiate defaults to false; enable it only for log-ratio effects to display ratios. Pooling always occurs on the input scale.
result is a coefficient-style plot with data (label, value, lower, upper) and confidenceLevel. Study labels are one-based row positions. All study intervals and the pooled row are retained; the plot contains no square-size weighting or diamond summary.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
