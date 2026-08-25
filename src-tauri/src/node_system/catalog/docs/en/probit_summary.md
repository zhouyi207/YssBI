# Probit Summary

Same inputs as **Probit**; after estimation emits a summary and opens the report window.

## Inputs

| Pin | Description |
|-----|-------------|
| **In** | Execution flow entry |
| **Y** | Binary dependent variable |
| **X** | One or more regressors |
| **Time** | Optional time index |
| **Config** | Optional **Probit Configure** |

## Output

| Pin | Description |
|-----|-------------|
| **Result** | Summary **OLSResult** |
| **Out** | Execution flow exit |

Opens the Probit report window after estimation. Use **Probit** for a reusable **ProbitModel** handle only.
