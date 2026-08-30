# Multiply (×)

Element-wise multiplication with scalar broadcast:

$$
\text{Result} = A \times B
$$

When either input is a `DataSeries`, both are promoted to `DataSeries<Float64>`; a scalar broadcasts to the series length. Two scalars yield `Float64`.

## Usage

Multiply a series by a scalar to scale values, or multiply two aligned series for element-wise products (e.g. interaction terms before regression).
