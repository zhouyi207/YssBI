# Sharp regression discontinuity

Estimates the outcome jump at a known treatment cutoff using separate local linear slopes.

## Inputs and parameters

Connect aligned numeric `Y` and `running` series. `rdd_cutoff=0`, `rdd_bandwidth=1` and `rdd_kernel=triangular` are the defaults.
Bandwidth must be finite and positive, in running-variable units. The uniform kernel is also available. Treatment starts at the cutoff, including equality. No bandwidth is automatically selected.

Triangular weighting uses observations with $|r_i-c|<h$ and weight $1-|r_i-c|/h$. Uniform weighting uses $|r_i-c|\le h$ with weight one. Each side needs enough distinct running values for a full-rank design and HC3 inference; leverage-one observations are rejected. Inputs must be complete and finite.

## Model and inference

For centered running value $x_i=r_i-c$ and $D_i=1[r_i\ge c]$, weighted least squares fits

$$
y_i=\alpha+\tau D_i+\beta x_i+\gamma D_i x_i+\epsilon_i.
$$

The discontinuity coefficient $\tau$ is the local treatment effect. HC3 sandwich covariance treats the selected bandwidth as fixed. Coefficient tests use $H_0:b_j=0$ versus $H_1:b_j\ne0$, $z=\hat b_j/SE_j$, asymptotic standard normal reference, two-sided p-values and 95% intervals.

A causal interpretation requires a sharp assignment rule, continuity of potential outcome means at the cutoff, and no precise manipulation of the running variable. This implementation has no fuzzy design, automatic bandwidth, bias correction or robust bias-corrected interval.

## Result

`coefficients` are ordered `intercept_left`, `discontinuity`, `slope_left`, `slope_change`.
`result` also contains covariance, left/right counts, cutoff, bandwidth, kernel, retained one-based `rows`, fitted values and residuals for those retained rows.
The right slope is $\beta+\gamma$; this is a local discontinuity, not a population ATE.
