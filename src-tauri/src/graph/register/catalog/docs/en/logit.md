# Logit

Binary logistic regression estimated by IRLS. For $y_i \in \{0,1\}$:

$$
P(y_i=1 \mid x_i) = \Lambda(x_i'\beta) = \frac{1}{1+e^{-x_i'\beta}}
$$

## Inputs

- **Y** — binary dependent variable (`Float64` / `Int64` / `Boolean` `DataSeries`)
- **X** — one or more regressors (`Float64` or `Categorical`)
- Optional **Config** from **Logit Configure**
- Optional **Time** (metadata)

## Outputs

- **Model** — **LogitModel** for **Logit Predict**
- **Fitted** — in-sample predicted probabilities
- **Residuals** — response minus fitted

Use **Logit Summary** for the full report window.
