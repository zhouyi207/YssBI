# I-squared heterogeneity

Connect effects and positive variances for at least two independent studies. With fixed-weight Cochran $Q$ and $df=k-1$, $I^2=100\max(0,(Q-df)/Q)$ (zero when $Q=0$), and $H^2=\max(1,Q/df)$.
result contains i_squared_percent, h_squared, q, degrees_of_freedom and p_value. That P value belongs to Cochran's homogeneity test, $Q\sim\chi^2_{k-1}$ under a common true effect, against heterogeneity; it is not an independent I² test. tau_squared is zero because no random-effects variance is fitted here. I² is relative heterogeneity, not an absolute effect variance.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
