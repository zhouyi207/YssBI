# Log2 (Base-2 Logarithm)

Base-2 logarithm, element-wise:

$$
\text{Result} = \log_2 x
$$

**Domain:** $x > 0$. Accepts `Int64`, `Float64`, or numeric `DataSeries`; output is `Float64` or `DataSeries<Float64>`.

## Usage

Use when measuring growth in doublings or bit-related scales. Same positivity constraint as **Ln**.
