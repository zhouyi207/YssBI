# Subtract (−)

Element-wise subtraction with scalar broadcast:

$$
\text{Result} = A - B
$$

When either input is a `DataSeries`, a scalar broadcasts to the series length and the result is a `DataSeries`. `Int64` is preserved unless either input is `Float64`; two scalar inputs produce a scalar.

## Usage

Connect **A** and **B** (or leave defaults). Subtract a scalar from a series for demeaning-style transforms, or subtract two aligned series element-wise.
