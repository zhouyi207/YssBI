# Moderation (three-way interaction)

Connect response Y, predictor X, moderator W and second_moderator Z, with optional covariates.
Includes all main effects, all pairwise interactions and X×W×Z.
The conditional slope of X is `bX + bXW×Wc + bXZ×Zc + bXWZ×Wc×Zc`, using centered Wc/Zc.
Reports nine slopes at a 3×3 probe grid and a W Johnson–Neyman region for each of the three Z probes.

Inputs must be aligned finite numeric columns; missing rows are not silently removed. X, moderators and covariates are mean-centered. `input_centers` records the means and coefficients retain original units.
Covariates enter additively without automatic interactions. The design must have full rank and observations must exceed the parameter count.

**result** contains OLS coefficients, classical homoskedastic covariance, 95% t intervals and fit statistics. **observations** is the complete paged observation/response/fitted/residual table, without a fixed row cap.

**details.effects** reports conditional slopes of X, standard errors, two-sided t tests and 95% intervals. `probe_sd` defaults to 1: probes use each moderator's mean and mean ± the multiplier times its sample SD, in original units.
`in_observed_ranges` identifies extrapolated probes.

**details.johnson_neyman** solves the two-sided 5% significance boundaries using the full coefficient covariance. Boundaries and significant ranges are clipped to the observed minimum/maximum of W.
At a boundary p=0.05; inside a significant interval p<0.05. No boundaries can mean either all or none of the observed range is significant; inspect `significant_ranges`.
This is pointwise inference without correction for searching multiple W values. Zero variance does not produce infinite test statistics.

These continuous linear interactions assume independent homoskedastic errors. Interaction associations do not establish causal moderation. Automatic categorical coding, robust/clustered covariance and nonlinear outcome models are not included.

[Reference: interactions simple slopes](https://interactions.jacob-long.com/reference/sim_slopes.html)
[Reference: Johnson–Neyman](https://interactions.jacob-long.com/reference/johnson_neyman.html)
