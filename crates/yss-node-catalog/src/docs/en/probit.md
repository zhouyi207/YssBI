# Probit

Binary probit regression (IRLS). For $y_i \in \{0,1\}$:

$$
P(y_i=1 \mid x_i) = \Phi(x_i'\beta)
$$

where $\Phi$ is the standard normal CDF.

Connect an aligned finite response and ordered predictors. The response must be 0 or 1. Intercept, maximum iterations (positive integer) and positive tolerance configure estimation. Defaults are an intercept, 100 iterations and tolerance 1e-8. Failed convergence is reported explicitly.

Outputs model, fitted probabilities and response-minus-probability residuals. Summary reads the existing model; Predict consumes the model and the same ordered predictor definitions, returning probabilities without refitting.

IRLS uses working response η+(y−p)/φ(η) and weights φ(η)²/[p(1−p)]. Default coefficient covariance is the inverse observed Bernoulli-probit information (OIM). Probit coefficients are not log-odds; no odds-ratio output is offered.
