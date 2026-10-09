# IV LIML Fit

Connect one response, optional ordered exogenous predictors, one or more endogenous regressors, and one or more excluded instruments. Every column must contain aligned, finite observations; do not include the intercept yourself.

The intercept defaults to enabled, covariance to nonrobust (HC0–HC3 are available), and `small` to false. Coefficient probabilities and 95% intervals use a normal reference by default; `small=true` applies the small-sample covariance adjustment and uses Student-t with `n-k` degrees, where `n` is observations and `k` includes the fitted intercept. The overall coefficient test uses chi-square by default or F with `small=true`; its distribution and degrees are retained in `statistics.modelTest`. Identification, rank and undefined coefficient-inference failures are reported explicitly.

Outputs model, fitted values and structural residuals. The model retains coefficients, inference, kappa and the fitted design. Connect it to IV LIML Summary to select first-stage or overidentification analyses; these are calculated when requested. Fitted values use the observed endogenous regressors.
