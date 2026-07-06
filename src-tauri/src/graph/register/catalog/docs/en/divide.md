# Divide (÷)

Element-wise division with scalar broadcast:

$$
\text{Result} = \frac{A}{B}, \quad B \neq 0
$$

When either input is a `DataSeries`, both are promoted to `DataSeries<Float64>`; a scalar broadcasts to the series length. Division by zero errors at runtime.

## Inputs

| Pin | Description |
|-----|-------------|
| **A** (optional) | Dividend: `Int64`, `Float64`, or numeric `DataSeries` |
| **B** (optional) | Divisor: same types as **A** |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | Quotient as `Float64` or `DataSeries<Float64>` |

## Usage

Divide a series by a scalar to normalize, or divide two aligned series (e.g. ratios). Ensure **B** has no zeros where division must be defined.
