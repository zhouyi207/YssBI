# Subtract (−)

Element-wise subtraction with scalar broadcast:

$$
\text{Result} = A - B
$$

When either input is a `DataSeries`, both are promoted to `DataSeries<Float64>`; a scalar broadcasts to the series length. Two scalars yield `Float64`.

## Usage

Connect **A** and **B** (or leave defaults). Subtract a scalar from a series for demeaning-style transforms, or subtract two aligned series element-wise.
