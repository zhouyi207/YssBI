# Prais Configure

Builds a **PraisConfigure** struct for **Prais** / **Prais Summary** to correct AR(1) serial correlation in the error term (Stata `prais`).

## Inputs

| Pin | Default | Options |
|-----|---------|---------|
| **Constant** | `true` | Intercept |
| **Transform** | `prais` | `prais` (Prais–Winsten), `corc` (Cochrane–Orcutt) |

## Output

| Pin | Description |
|-----|-------------|
| **Config** | **PraisConfigure** handle |

Wire **Config** to the optional **Config** pin on **Prais** / **Prais Summary**.
