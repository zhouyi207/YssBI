# Square

Square, element-wise:

$$
\text{Result} = x^2
$$

Defined for all real $x$. Accepts `Int64`, `Float64`, or numeric `DataSeries`; output is `Float64` or `DataSeries<Float64>`.

Scalars execute directly; materialized series execute element-wise and lazy series execute in batches when consumed. Nulls, invalid domains, non-finite inputs/results, and integers that cannot widen exactly to Float64 fail instead of silently producing null. The source dataset is unchanged.

## Usage

Build polynomial terms or variance proxies. Prefer **Square** over **Multiply** when squaring a single input.
