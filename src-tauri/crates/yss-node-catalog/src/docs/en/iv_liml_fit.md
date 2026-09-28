# IV LIML Fit

Connect one response, optional ordered exogenous predictors, one or more endogenous regressors, and one or more excluded instruments. Every column must contain aligned, finite observations; do not include the intercept yourself.

Configure the intercept, covariance (nonrobust or HC0–HC3), and small-sample adjustment. Identification and rank failures are reported explicitly.

Outputs model, fitted values and structural residuals. The model retains coefficients, inference, kappa and the fitted design. Connect it to IV LIML Summary to select first-stage or overidentification analyses; these are calculated when requested. Fitted values use the observed endogenous regressors.
