# Log10 (Common Logarithm)

Base-10 logarithm, element-wise:

$$
\text{Result} = \log_{10} x
$$

**Domain:** $x > 0$. Accepts `Int64`, `Float64`, or numeric `DataSeries`; output is `Float64` or `DataSeries<Float64>`.

Scalars execute directly; materialized series execute element-wise and lazy series execute in batches when consumed. Nulls, invalid domains, non-finite inputs/results, and integers that cannot widen exactly to Float64 fail instead of silently producing null. The source dataset is unchanged.

## Usage

Common for orders-of-magnitude transforms (decibels-style scaling, log axes). Requires strictly positive inputs.
