# Probit

Binary probit regression (IRLS). For $y_i \in \{0,1\}$:

$$
P(y_i=1 \mid x_i) = \Phi(x_i'\beta)
$$

where $\Phi$ is the standard normal CDF.

## Inputs

- **Y** — binary dependent variable
- **X** — regressors (`Float64` or `Categorical`)
- Optional **Config**, optional **Time**

## Outputs

- **Model** — **ProbitModel** for **Probit Predict**
- **Fitted** / **Residuals**

Use **Probit Summary** for the report window.
