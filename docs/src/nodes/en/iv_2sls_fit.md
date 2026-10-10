# IV 2SLS Fit

Connect one response, optional ordered exogenous predictors, one or more endogenous regressors, and one or more excluded instruments. Every column must contain aligned, finite observations; do not include the intercept yourself.

The intercept defaults to enabled, covariance to nonrobust (HC0–HC3 are available), and `small` to false. Coefficient probabilities and 95% intervals use a normal reference by default; `small=true` applies the small-sample covariance adjustment and uses Student-t with `n-k` degrees, where `n` is observations and `k` includes the fitted intercept. The overall coefficient test uses chi-square by default or F with `small=true`; its distribution and degrees are retained in `statistics.modelTest`. Identification, rank and undefined coefficient-inference failures are reported explicitly.

The model records $R^2=1-\mathrm{RSS}/\mathrm{TSS}$ and $\bar R^2=1-(1-R^2)(n-c)/(n-k)$, where $c=1$ with an intercept and $c=0$ otherwise. RSS uses structural residuals; TSS uses centered observations with an intercept and squared observations without one. Negative IV R² is valid. Response-unit changes retain these ratios; undefined total variation fails the fit.

Outputs model, fitted values and structural residuals. The model retains coefficients, inference and the fitted design. Connect it to IV 2SLS Summary to select first-stage, overidentification or endogeneity analyses; these are calculated when requested. Fitted values use the observed endogenous regressors.
