# Histogram

Values requires at least one finite Numeric observation and rejects missing values. Bins defaults to 0, selecting $k=\lceil\log_2 n\rceil+1$ by Sturges' rule, capped at 128. Set 1–128 to choose a bin count explicitly.

Bins have equal width and are left-inclusive/right-exclusive except the final bin, which includes the maximum. Constant samples expand around their value. Counts sum to the complete observation count. Result includes endpoints and counts and opens in a result panel or Plot window.
