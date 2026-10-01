# Random effects (RE)

Inputs are aligned finite numeric response, one or more predictors, entity IDs and time. Missing values and duplicate entity-time keys are rejected; rows are sorted. Encode textual IDs first.

**Parameters:** constant defaults to true; effects defaults to entity, with time and two_way alternatives; covariance defaults to nonrobust, with HC0–HC3 and entity-clustered cluster alternatives. This entry uses feasible GLS (FGLS); select maximum likelihood in the general Panel Fit node when required.

The entity model is $y_{it}=\alpha+x_{it}'\beta+u_i+\varepsilon_{it}$, assuming random effects are uncorrelated with regressors. FGLS estimates variance components and quasi-demeans observations. time and two_way select time or two-way random effects.

The only output, **model**, connects to Panel Summary and estimation-scale prediction. It retains coefficients, covariance, variance components, the quasi-demeaned sample and source-row groups. Coefficient tests use $z_j=\hat\beta_j/SE(\hat\beta_j)$ with a two-sided standard-normal reference and a zero-coefficient null. Estimator statistics include the overall Wald test and coefficient intervals are 95% normal intervals.

Insufficient within/between variation, unidentified designs or estimation failure produce errors. The retained sample is on the quasi-demeaned scale and does not predict random effects for new entities.
