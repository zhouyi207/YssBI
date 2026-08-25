# Multiply (×)

Element-wise multiplication with scalar broadcast:

$$
\text{Result} = A \times B
$$

When either input is a `DataSeries`, both are promoted to `DataSeries<Float64>`; a scalar broadcasts to the series length. Two scalars yield `Float64`.

## Inputs

| Pin | Description |
|-----|-------------|
| **A** (optional) | First factor: `Int64`, `Float64`, or numeric `DataSeries` |
| **B** (optional) | Second factor: same types as **A** |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | Product as `Float64` or `DataSeries<Float64>` |

## Usage

Multiply a series by a scalar to scale values, or multiply two aligned series for element-wise products (e.g. interaction terms before regression).
