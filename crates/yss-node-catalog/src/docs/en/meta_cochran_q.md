# Cochran Q heterogeneity test

Connect effects and positive variances for at least two independent studies. With $w_i=1/v_i$ and fixed pooled mean $\hat\mu$, $Q=\sum w_i(y_i-\hat\mu)^2$ approximately follows $\chi^2_{k-1}$ under the null of a common true effect. The alternative is between-study heterogeneity; a small P value rejects homogeneity.
result contains q, degrees_of_freedom, p_value, i_squared_percent, h_squared and tau_squared (zero for this fixed-weight diagnostic). Q can have low power with few studies and high power with many studies; interpret magnitude alongside I² and study differences.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
