# Ln (Natural Logarithm)

Natural logarithm, element-wise:

$$
\text{Result} = \ln x
$$

**Domain:** $x > 0$. Non-positive inputs fail for both scalars and series. Accepts Numeric scalars or series and preserves input shape, with Float64 results.

Scalars execute directly; materialized series execute element-wise and lazy series execute in batches when consumed. Nulls, invalid domains, non-finite inputs/results, and integers that cannot widen exactly to Float64 fail instead of silently producing null. The source dataset is unchanged.

## Usage

Log-transform strictly positive series (e.g. income, prices). Filter or clip non-positive values upstream if needed.
