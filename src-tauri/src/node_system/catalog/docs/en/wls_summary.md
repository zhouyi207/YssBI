# WLS Summary

Same inputs as **WLS**; after estimation emits the full result and opens the report window.

## Inputs

| Pin | Description |
|-----|-------------|
| **In** | Execution flow entry |
| **Y** | Dependent variable |
| **X** | One or more regressors |
| **Weights** | Positive weights as a `Float64` `DataSeries` (same length as **Y**) |
| **Time** | Optional time index |
| **Config** | Optional **OLS & WLS Configure** (intercept, VCE) |

## Output

| Pin | Description |
|-----|-------------|
| **Result** | Full **OLSResult** (WLS estimates with chosen VCE) |
| **Out** | Execution flow exit |

Automatically opens the **OLS Summary** result window. Use **WLS** instead if you only need a reusable **Model** handle without opening the summary window.
