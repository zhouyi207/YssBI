# Spatially lagged X (SLX)

Adds first-order spatial lags of every predictor to OLS; the intercept is not spatially lagged.

## Inputs and settings

- `weights`: the design object from Spatial weights.
- `units`: unique observation identifiers, exactly matching the weights' unit set. Computation aligns by identifier and observation arrays return in input row order. Text and numeric identifiers remain distinct; wide integers remain exact.
- `Y`: finite numeric response; `X₁, X₂, …`: one or more finite numeric columns with equal lengths, paired by current position. Missing values are rejected rather than dropped.
- `constant`: true by default. Do not supply another constant column.

## Model and inference

$$
y=X\beta+WX_*\theta+\varepsilon
$$

$W$ is the supplied weights matrix; $X$ includes the selected intercept, while $X_*$ contains only original predictors. Innovations $\varepsilon$ are independent, homoskedastic Gaussian errors. Predictors are treated as exogenous; no automatic endogeneity or heteroskedasticity correction is applied. The expanded design must have full column rank and observations must exceed the number of regression and spatial coefficients.

For each coefficient, test $H_0:b_j=0$ against $H_1:b_j\ne0$ using $t=\hat b_j/SE(\hat b_j)$ and Student $t_{n-p}$, where $p$ counts design columns including the intercept and lagged predictors. Report two-sided p-values and 95% intervals; residual variance uses $RSS/(n-p)$.

## Results

`coefficients` contains original-scale estimates, standard errors, statistics, p-values and intervals. `covariance` follows coefficient order and excludes the error variance. Lagged predictors use `W:column name`; spatial parameters use `rho` and `lambda`. Fit fields include `sigma_squared`, `log_likelihood`, `aic`, `bic`, `df_residual` and `iterations`; information-criterion parameter counts include error variance.

- `fitted` includes observed $Wy$; `residuals=y-fitted`.
- `innovations=(I-\lambda W)residuals`; `reduced_fitted=(I-\rho W)^{-1}(X\hat\beta+WX_*\hat\theta)` gives in-sample reduced-form predictions without substituting observed $Wy$.
- `impacts` gives average direct, indirect and total point effects for original predictors. The effect matrix is $(I-\rho W)^{-1}(\beta_k I+\theta_k W)$: direct is its average diagonal, total its average row sum, and indirect their difference. Absent parameters are zero. No effect p-values are estimated.
- `innovation_moran_i` is descriptive Moran I for innovations; constant innovations yield null. Raw-data randomization inference is not applied to regression residuals.
- `unit_labels` follows weights order; `observation_units` contains zero-based label indices for each input row. For a cross section, `period_labels`, `observation_periods` and `unit_effects` are empty, and `periods=1`.

Islands retain zero rows without self-neighbors. Dense calculations are admitted according to the execution memory budget.

[Spatial model reference](https://r-spatial.github.io/spatialreg/reference/ML_models.html)
