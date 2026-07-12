# Square

Square, element-wise:

$$
\text{Result} = x^2
$$

Defined for all real $x$. Accepts `Int64`, `Float64`, or numeric `DataSeries`; output is `Float64` or `DataSeries<Float64>`.

## Inputs

| Pin | Description |
|-----|-------------|
| **X** | Value or `DataSeries` |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | $x^2$ per element |

## Usage

Build polynomial terms or variance proxies. Prefer **Square** over **Multiply** when squaring a single input.
