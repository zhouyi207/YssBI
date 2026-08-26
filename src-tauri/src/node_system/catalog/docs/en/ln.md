# Ln (Natural Logarithm)

Natural logarithm, element-wise:

$$
\text{Result} = \ln x
$$

**Domain:** $x > 0$. Non-positive inputs yield null (series) or an error (scalar). Accepts `Int64`, `Float64`, or numeric `DataSeries`; output is `Float64` or `DataSeries<Float64>`.

## Usage

Log-transform strictly positive series (e.g. income, prices). Filter or clip non-positive values upstream if needed.
