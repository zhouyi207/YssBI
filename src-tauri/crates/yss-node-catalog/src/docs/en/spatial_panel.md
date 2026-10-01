# Spatial panel: entity fixed effects

Remove entity fixed effects using orthonormal time contrasts, then fit a Gaussian spatial lag or spatial error model to a balanced panel. Spatial weights are constant over periods.

## Inputs and parameters

- `weights`: a one-row-per-unit spatial weights design; do not construct weights from repeated panel rows.
- `units`, `periods`: each unit-period combination must occur exactly once and every period must cover all weighted units. Input sorting is unnecessary; period labels follow first appearance.
- `response` and one or more `predictors`: aligned finite numeric columns. Missing, duplicate or unbalanced observations are rejected. Time-invariant or collinear predictors are unidentified after removing entity effects and must be removed.
- `spatial_panel_model`: `slm` (default) or `sem`.
- `max_iterations`: 500 by default, at least 1; `tolerance`: $10^{-7}$ by default, in $[10^{-12},0.01]$. At least two units and two periods are required; no additional overall intercept is fitted.

## Model and inference

SLM: $y_t=\rho Wy_t+X_t\beta+\alpha+\varepsilon_t$. SEM: $y_t=X_t\beta+\alpha+u_t$, $u_t=\lambda Wu_t+\varepsilon_t$. Entity effects $\alpha$ are constant over time; innovations are independent homoskedastic Gaussian errors.

Orthonormal Helmert contrasts satisfy $H\mathbf1=0$ and $HH^T=I_{T-1}$. The transformed sample has $N(T-1)$ observations and log determinant $(T-1)\log|I-\rho W|$ or $(T-1)\log|I-\lambda W|$. Results use this transformed likelihood; information criteria exclude eliminated entity effects from free parameter counts and are not directly comparable with uncorrected likelihoods using $NT$ demeaned observations.

Spatial parameters have absolute value below $1/\max_i\sum_jw_{ij}$, a possibly conservative interval for unstandardized weights. Joint observed information supplies covariance. Test $H_0:b_j=0$ against $H_1:b_j\ne0$ using asymptotic standard-normal $z=\hat b_j/SE(\hat b_j)$, two-sided p-values and 95% Wald intervals. Effective observations must exceed regression and spatial parameter counts. No individual entity-effect significance tests are produced.

## Results and scope

`coefficients`, `covariance`, `rho` or `lambda`, `sigma_squared`, `log_likelihood`, `aic`, `bic`, `df_residual` and `iterations` describe the transformed model. `observations=NT`, `estimation_observations=N(T-1)`, `periods=T`.

`unit_labels` follows weights order and `period_labels` follows first appearance. `observation_units` and `observation_periods` are zero-based label indices preserving input order. `unit_effects` restores $\hat\alpha_i$ in weights order. `fitted` includes entity effects and the observed spatial response lag; `residuals=y-fitted`; `innovations` applies the spatial error filter; `reduced_fitted` uses the feedback inverse and includes entity effects. These four arrays return in input row order.

`impacts` gives average direct, indirect and total point effects. SLM uses $(I-\rho W)^{-1}\beta_k I$; SEM has direct and total effects $\beta_k$ and zero indirect effect. `innovation_moran_i` contains descriptive indices by period without residual-test p-values.

Supports static entity fixed effects, excluding random effects, two-way effects, temporal response dynamics and time-varying weights. Execution budgets apply and failures/nonconvergence do not return partial fits.

[Spatial model definitions](https://r-spatial.github.io/spatialreg/reference/ML_models.html)
