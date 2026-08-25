# Logit Summary

Same inputs as **Logit**; after estimation emits a summary and opens the report window.

## Inputs

| Pin | Description |
|-----|-------------|
| **In** | Execution flow entry |
| **Y** | Binary dependent variable (`Float64` / `Int64` / `Boolean` `DataSeries`) |
| **X** | One or more regressors |
| **Time** | Optional time index (metadata) |
| **Config** | Optional **Logit Configure** |

## Output

| Pin | Description |
|-----|-------------|
| **Result** | **OLSResult**-shaped summary (logit coefficients, diagnostics) |
| **Out** | Execution flow exit |

Opens the summary result window after estimation. Use **Logit** if you only need a **LogitModel** handle for **Logit Predict**.
