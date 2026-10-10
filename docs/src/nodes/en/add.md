# Add (+)

Element-wise addition with scalar broadcast:

$$
\text{Result} = a + b + \cdots
$$

When any operand is a `DataSeries`, scalars broadcast to the series length and the result is a `DataSeries`. `Int64` is preserved unless at least one operand is `Float64`; two scalar operands produce a scalar by the same promotion rule.

## Usage

Chain two or more values or series into the operand pins. Mix a constant scalar with a series to add an offset column-wise. Shape and element type are solved independently: series wins over scalar, and `Float64` wins over `Int64`.
