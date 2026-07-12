# Equal (==)

Tests whether two values are equal:

$$
\text{Result} = (A = B)
$$

Compares scalar `Float64` operands using value equality. Output is a single `Boolean` (not a `DataSeries`).

## Inputs

| Pin | Description |
|-----|-------------|
| **A** (optional) | First `Float64` operand |
| **B** (optional) | Second `Float64` operand |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | `true` if **A** equals **B**, else `false` |

## Usage

Drive **Branch** conditions or combine with **And** / **Or**. For series-wise comparison, use dedicated **DataSeries** compare nodes.
