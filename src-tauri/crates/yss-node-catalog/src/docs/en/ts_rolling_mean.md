# Time-Series Rolling Mean

Window is a positive trailing row count, including the current row. Operation defaults to mean; sum, minimum, maximum and sample standard deviation (ddof = 1) are also available. Min periods defaults to 0, requiring a complete non-null window; otherwise it specifies the minimum non-null count, from 1 to Window. Insufficient observations produce Null; standard deviation needs at least two.

By default, use current row order. Optional Context must share the series row domain and provides partition and order columns, with descending and null placement options. Output is Float64 and retains the original row order and alignment.
