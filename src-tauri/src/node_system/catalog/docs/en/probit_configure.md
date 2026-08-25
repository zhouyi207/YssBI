# Probit Configure

Builds a **ProbitConfigure** struct for **Probit** / **Probit Summary**.

## Inputs

| Pin | Description |
|-----|-------------|
| **Constant** | Include intercept (default `true` when unconnected) |

## Output

| Pin | Description |
|-----|-------------|
| **Config** | **ProbitConfigure** handle |

Wire **Config** to the optional **Config** pin on Probit nodes. When unconnected, the regression node uses built-in defaults.
