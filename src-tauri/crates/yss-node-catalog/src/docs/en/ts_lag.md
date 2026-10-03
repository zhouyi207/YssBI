# Lag Time Series

Window is a positive number of rows. Direction defaults to lag, filling leading positions with Null; lead reads following rows and fills trailing positions with Null. Preserves element type and semantic metadata. A displacement at least as large as its group produces all Null values.

By default, use current row order. Optional Context must have the same row count and pairs with the series by current position. It provides partition and order columns, with descending and null placement options. Output retains original row order. Displacement counts rows and does not fill calendar gaps.
