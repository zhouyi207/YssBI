# Equal (==)

Tests whether two values are equal:

$$
\text{Result} = (A = B)
$$

Compares two values with a shared input type. Numeric values compare exactly across signed integer, unsigned integer and floating representations, without epsilon or lossy integer conversion. Lists and records compare their contents using the same rules; other values compare by value or resource identity. Output is a single `Boolean` (not a `DataSeries`).

## Usage

Produce a Boolean condition for filters or combine it with **And** / **Or**. For series-wise comparison, use dedicated **DataSeries** compare nodes.
