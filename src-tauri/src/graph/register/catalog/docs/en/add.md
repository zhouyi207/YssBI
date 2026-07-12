# Add (+)

Element-wise addition with scalar broadcast:

$$
\text{Result} = a + b + \cdots
$$

When any operand is a `DataSeries`, all operands are promoted to `DataSeries<Float64>`; scalars broadcast to the series length. Two scalars yield a `Float64` scalar.

## Inputs

| Pin | Description |
|-----|-------------|
| **Operands** (≥2, optional) | `Int64`, `Float64`, or numeric `DataSeries`; unnamed repeatable pins |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | Sum as `Float64` or `DataSeries<Float64>` |

## Usage

Chain two or more values or series into the operand pins. Mix a constant scalar with a series to add an offset column-wise. Result type follows the most general input (series wins over scalar).
