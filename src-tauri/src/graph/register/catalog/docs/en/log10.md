# Log10 (Common Logarithm)

Base-10 logarithm, element-wise:

$$
\text{Result} = \log_{10} x
$$

**Domain:** $x > 0$. Accepts `Int64`, `Float64`, or numeric `DataSeries`; output is `Float64` or `DataSeries<Float64>`.

## Inputs

| Pin | Description |
|-----|-------------|
| **X** | Value or `DataSeries` to transform |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | $\log_{10} x$ per element |

## Usage

Common for orders-of-magnitude transforms (decibels-style scaling, log axes). Requires strictly positive inputs.
