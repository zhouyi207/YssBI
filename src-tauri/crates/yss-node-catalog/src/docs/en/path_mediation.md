# Mediation (one observed mediator)

Connect response Y, predictor X and mediator M, optionally followed by continuous covariates C. Both OLS equations include an intercept:

$$M=i_M+aX_c+g^TC_c+e_M,\qquad Y=i_Y+c'X_c+bM_c+h^TC_c+e_Y.$$

Subscript c denotes sample mean centering. `input_centers` follows X, M, covariate order; coefficients retain original units. Direct, indirect and total effects are c′, ab and c′+ab. Covariates enter both equations.

Columns must be aligned, finite and complete; no rows are dropped. Each design must have full rank and more observations than parameters.

`replications` defaults to 1000 and `seed` to 42 (nonnegative integer). Zero replications returns effect point estimates only; otherwise at least two are required. Each replication resamples complete rows with replacement and refits both equations using fixed observed centers and probe coordinates. Any failed fit aborts inference instead of being discarded. More replications generally improve percentile stability. There is no fixed row ceiling.

**result** contains observation count, input names, coefficients/covariance/fit statistics for both equations, and direct, indirect and total effects under `details.direct` and `details.effects`. Equation coefficients have classical homoskedastic OLS standard errors, two-sided zero-coefficient t tests and 95% intervals with n−k residual degrees of freedom. Effect inference uses an iid paired-row bootstrap: standard errors are sample SDs of estimates; 95% intervals use Type-7 2.5th/97.5th percentiles. No normal-approximation p values are reported. An indirect-effect interval excluding zero is interval evidence against a zero indirect effect.

**observations** retains every row in a paged table: `observation, response, fitted, residual, mediator, mediator_fitted, mediator_residual`; observation indices start at 1.

Supports one continuous observed mediator and a continuous linear outcome, without serial/multiple mediators, latent variables, cluster resampling or nonlinear links. Causal interpretation additionally requires appropriate temporal ordering, confounding control and model assumptions.

[Reference: lavaan mediation and effect definitions](https://lavaan.ugent.be/tutorial/mediation.html)
