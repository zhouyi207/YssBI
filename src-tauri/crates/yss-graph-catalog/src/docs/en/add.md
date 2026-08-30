# Add (+)

Element-wise addition with scalar broadcast:

$$
\text{Result} = a + b + \cdots
$$

When any operand is a `DataSeries`, all operands are promoted to `DataSeries<Float64>`; scalars broadcast to the series length. Two scalars yield a `Float64` scalar.

## Usage

Chain two or more values or series into the operand pins. Mix a constant scalar with a series to add an offset column-wise. Result type follows the most general input (series wins over scalar).
