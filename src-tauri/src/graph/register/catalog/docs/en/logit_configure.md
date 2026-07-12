# Logit Configure

Builds a **LogitConfigure** struct for **Logit** / **Logit Summary**.

## Inputs

| Pin | Description |
|-----|-------------|
| **Constant** | Include intercept (default `true` when unconnected) |

## Output

| Pin | Description |
|-----|-------------|
| **Config** | **LogitConfigure** handle |

Wire **Config** to the optional **Config** pin on Logit nodes. When unconnected, the regression node uses built-in defaults.
