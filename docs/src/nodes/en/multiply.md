# Multiply (×)

Element-wise multiplication with scalar broadcast:

$$
\text{Result} = A \times B
$$

When either input is a `DataSeries`, a scalar broadcasts to the series length and the result is a `DataSeries`. `Int64` is preserved unless either input is `Float64`; two scalar inputs produce a scalar.

## Usage

Multiply a series by a scalar to scale values, or multiply two aligned series for element-wise products (e.g. interaction terms before regression).
