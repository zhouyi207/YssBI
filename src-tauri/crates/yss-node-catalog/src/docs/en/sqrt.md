# Sqrt (Square Root)

Square root, element-wise:

$$
\text{Result} = \sqrt{x}
$$

**Domain:** $x \geq 0$, including zero. Negative inputs fail for both scalars and series. Accepts Numeric scalars or series and preserves input shape, with Float64 results.

Scalars execute directly; materialized series execute element-wise and lazy series execute in batches when consumed. Nulls, invalid domains, non-finite inputs/results, and integers that cannot widen exactly to Float64 fail instead of silently producing null. The source dataset is unchanged.

## Usage

Transform variance-like or squared quantities back to original units. Clip negatives upstream when needed.
