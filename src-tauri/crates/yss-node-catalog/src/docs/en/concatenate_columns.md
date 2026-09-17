# Concatenate Columns

Combine columns from two or more lazy DataFrames in input-port order. Duplicate names receive suffixes such as `_2` and `_3`.

Inputs must provably share one row domain, for example column selections or renames of the same DataFrame. Equal row counts do not prove alignment. Use Join with explicit keys for independent tables or separately filtered/limited inputs.

The result preserves row order and evaluates its projection when consumed, without modifying source datasets.
