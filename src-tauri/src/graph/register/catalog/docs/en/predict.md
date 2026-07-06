# Predict

Applies a fitted **OLSModel** (from **OLS**, **WLS**, or **GLS**) to new exogenous data.

## Inputs

- **Model** — **OLSModel** (or compatible model handle) from an upstream regression node
- **Exog pins** — created dynamically from the model's training regressors (names and types match estimation)

Each exog pin must be a `DataSeries` with the same length; categorical columns use the same encoding as at fit time.

## Output

**Predicted** — `DataSeries<Float64>` of fitted values $\hat y = X_{\mathrm{new}} \hat\beta$.

Connect **Model** first; input pins appear after the model pin is wired.
