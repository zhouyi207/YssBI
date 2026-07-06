# Prais Summary

Same inputs as **Prais**; after estimation emits the result and opens the Prais summary window.

## Inputs

| Pin | Description |
|-----|-------------|
| **In** | Execution flow entry |
| **Y** | Dependent variable |
| **X** | One or more regressors |
| **Time** | Strongly recommended (observation order) |
| **Config** | Optional **Prais Configure** (**Transform**: `prais` or `corc`) |

## Output

| Pin | Description |
|-----|-------------|
| **Result** | **OLSResult** (includes $\hat\rho$, transform type, and other diagnostics) |
| **Out** | Execution flow exit |

Opens the Prais summary window after estimation. Use **Prais** for a **PraisModel** handle only.
