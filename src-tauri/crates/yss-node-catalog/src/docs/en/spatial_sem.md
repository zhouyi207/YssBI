# Spatial error model (SEM)

Gaussian maximum likelihood with spatially correlated disturbances.

## Inputs and settings

- `weights`: the design object from Spatial weights.
- `units`: unique observation identifiers, exactly matching the weights' unit set. Computation aligns by identifier and observation arrays return in input row order. Text and numeric identifiers remain distinct; wide integers remain exact.
- `response`: finite numeric response; `predictors`: one or more finite numeric columns sharing the same row domain. Missing values are rejected rather than dropped.
- `constant`: true by default. Do not supply another constant column.
- `max_iterations`: 500 by default, at least 1; `tolerance`: $10^{-7}$ by default, in $[10^{-12},0.01]$. Nonconvergence or an estimate approaching the stability boundary is a failure.

## Model and inference

$$
y=X\beta+u,\quad u=\lambda Wu+\varepsilon
$$

$W$ is the supplied weights matrix; $X$ includes the selected intercept, while $X_*$ contains only original predictors. Innovations $\varepsilon$ are independent, homoskedastic Gaussian errors. Predictors are treated as exogenous; no automatic endogeneity or heteroskedasticity correction is applied. The expanded design must have full column rank and observations must exceed the number of regression and spatial coefficients.

The Gaussian likelihood includes spatial log determinants. Spatial parameters lie in the stable interval $|\rho|,|\lambda|<1/\max_i\sum_jw_{ij}$; unused parameters are zero. For unstandardized weights this interval can be more conservative than the full invertibility region. Innovation variance uses squared innovations divided by $n$. Covariance inverts the full observed information, accounting jointly for regression coefficients, spatial parameters and variance. Test $H_0:b_j=0$ against $H_1:b_j\ne0$ with asymptotic standard-normal $z=\hat b_j/SE(\hat b_j)$; report two-sided p-values and 95% Wald intervals.

## Results

`coefficients` contains original-scale estimates, standard errors, statistics, p-values and intervals. `covariance` follows coefficient order and excludes the error variance. Lagged predictors use `W:column name`; spatial parameters use `rho` and `lambda`. Fit fields include `sigma_squared`, `log_likelihood`, `aic`, `bic`, `df_residual` and `iterations`; information-criterion parameter counts include error variance.

- `fitted` includes observed $Wy$; `residuals=y-fitted`.
- `innovations=(I-\lambda W)residuals`; `reduced_fitted=(I-\rho W)^{-1}(X\hat\beta+WX_*\hat\theta)` gives in-sample reduced-form predictions without substituting observed $Wy$.
- `impacts` gives average direct, indirect and total point effects for original predictors. The effect matrix is $(I-\rho W)^{-1}(\beta_k I+\theta_k W)$: direct is its average diagonal, total its average row sum, and indirect their difference. Absent parameters are zero. No effect p-values are estimated.
- `innovation_moran_i` is descriptive Moran I for innovations; constant innovations yield null. Raw-data randomization inference is not applied to regression residuals.
- `unit_labels` follows weights order; `observation_units` contains zero-based label indices for each input row. For a cross section, `period_labels`, `observation_periods` and `unit_effects` are empty, and `periods=1`.

Islands retain zero rows without self-neighbors. Dense calculations are admitted according to the execution memory budget.

[Spatial model reference](https://r-spatial.github.io/spatialreg/reference/ML_models.html)
