# GLS Configure

Builds a **GLSConfigure** struct for **GLS** / **GLS Summary**.

## Inputs

| Pin | Description |
|-----|-------------|
| **Constant** | Include intercept (default `true` when unconnected) |
| **Time** | Optional time index (`Int64` or `Date` `DataSeries`) for diagnostics and report metadata |

## Output

| Pin | Description |
|-----|-------------|
| **Config** | **GLSConfigure** handle |

Wire **Config** to the optional **Config** pin on **GLS** / **GLS Summary**. When unconnected, the regression node uses built-in defaults (intercept included).
