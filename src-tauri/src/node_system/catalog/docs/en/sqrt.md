# Sqrt (Square Root)

Square root, element-wise:

$$
\text{Result} = \sqrt{x}
$$

**Domain:** $x \geq 0$. Negative inputs yield null (series) or NaN/error depending on context. Accepts `Int64`, `Float64`, or numeric `DataSeries`.

## Usage

Transform variance-like or squared quantities back to original units. Clip negatives upstream when needed.
