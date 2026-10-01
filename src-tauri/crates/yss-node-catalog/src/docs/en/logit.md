# Logit

Binary logistic regression estimated by IRLS. For $y_i \in \{0,1\}$:

$$
P(y_i=1 \mid x_i) = \Lambda(x_i'\beta) = \frac{1}{1+e^{-x_i'\beta}}
$$

Connect an aligned finite response and ordered predictors. The response must be 0 or 1. Intercept, maximum iterations (positive integer) and positive tolerance configure estimation. Defaults are an intercept, 100 iterations and tolerance 1e-8. Failed convergence is reported explicitly.

Outputs model, fitted probabilities and response-minus-probability residuals. Summary reads the existing model; Predict consumes the model and the same ordered predictor definitions, returning probabilities without refitting.
