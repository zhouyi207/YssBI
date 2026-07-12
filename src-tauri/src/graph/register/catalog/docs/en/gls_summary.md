# GLS Summary

Same inputs as **GLS**; after estimation emits the full result and opens the report window.

## Inputs

| Pin | Description |
|-----|-------------|
| **In** | Execution flow entry |
| **Y** | Dependent variable (`Float64` `DataSeries`) |
| **X** | One or more regressors (`Float64` or `Categorical`) |
| **Sigma** | n×n error covariance `DataFrame` |
| **Time** | Optional time index |
| **Config** | Optional **GLS Configure** |

## Output

| Pin | Description |
|-----|-------------|
| **Result** | **OLSResult** (GLS coefficients and diagnostics) |
| **Out** | Execution flow exit |

Automatically opens the **OLS Summary** result window. Use **GLS** instead if you only need a **Model** handle for **Predict**.
