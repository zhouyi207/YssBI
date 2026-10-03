# Survey linear regression

Connect response and sampling weights, optionally adding one strata column and one clusters (primary sampling unit, PSU) column.

Omitted strata means one stratum; omitted clusters treats every row as its own PSU. Identical PSU labels in different strata identify separate units. Columns must be aligned and complete, numbers finite, and sampling weights strictly positive. No rows are dropped.

Variance uses single-stage, with-replacement Taylor linearization, without finite-population corrections, multistage designs, replicate weights or subpopulation analysis. If stratum h has m_h PSUs and t_hj is a PSU score sum, variance sums `m_h/(m_h−1) × Σ(t_hj−stratum mean score)²` over strata. Regression uses outer-product matrices with inverse information on both sides. Design degrees of freedom equal PSU count minus stratum count.

`lonely_psu=fail` rejects single-PSU strata by default. Use `certainty` only for known certainty strata with no omitted later sampling stages; their variance contribution is zero. Overall design degrees of freedom must remain positive.

Weights are internally rescaled to mean 1. Multiplication by a common constant does not change estimates or standard errors. Design summaries retain original weight sum/range and Kish effective sample size `(Σw)²/Σw²`. `weight_design_effect=n/Kish effective sample size` describes unequal weighting alone, not the complete impact of stratification/clustering. There is no fixed row ceiling.

Continuous response, Gaussian identity-link pseudo-likelihood; point estimates equal weighted least squares.

Append numeric predictors; `constant=true` includes an intercept by default. Designs require full rank and more observations than regression parameters. `max_iterations` defaults to 500 (positive integer); `tolerance` defaults to 1e-7, within 1e-12 through 0.01. Nonconvergence fails.

**result** returns coefficients, survey sandwich covariance, iteration information and `details.design`; `factor_names` follows only predictor order. With k regression parameters, coefficient inference df is design df + 1 − k and must be positive. Two-sided zero-coefficient tests divide estimates by design standard errors, using t inference and 95% intervals. Ordinary GLM model-based variance and likelihood AIC/BIC are not used.

**observations** retains every `observation, response, fitted, residual` row. Residuals subtract response-scale fitted means from responses; indices start at 1.

[Reference: survey::svyglm](https://r-survey.r-forge.r-project.org/pkgdown/docs/reference/svyglm.html)
