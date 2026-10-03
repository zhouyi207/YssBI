# Begg rank-correlation test

Connect effects and positive variances for at least three independent studies. Let $\hat\mu$ be the fixed-weight pooled effect and $V=1/\sum1/v_i$. The test correlates $z_i=(y_i-\hat\mu)/\sqrt{v_i-V}$ with $v_i$ using Kendall's tau-b.
The null is no rank association between adjusted effects and variances; the alternative is a nonzero association. A tie-adjusted asymptotic normal test is used, without an exact permutation calculation or continuity correction. Both rank variables need variation.
result contains coefficient, observations, inference (statistic, p_value and method), concordant_pairs, discordant_pairs and tied-pair counts. This is a small-study asymmetry diagnostic; rejection does not prove publication bias.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
