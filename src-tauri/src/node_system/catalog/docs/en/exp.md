# Exp (Exponential)

Natural exponential, element-wise:

$$
\text{Result} = e^x
$$

Defined for all real $x$. Accepts `Int64`, `Float64`, or numeric `DataSeries`; output is `Float64` or `DataSeries<Float64>`.

## Usage

Invert a log transform or compute growth factors. Very large $|x|$ may overflow to infinity.
