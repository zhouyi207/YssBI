# Int64 to Categorical

Encodes `DataSeries<Int64>` as `DataSeries<Categorical>` via string representation (same category pool as other cat casts).

## Usage

Treat integer codes as unordered factors for regression. Distinct integers become distinct categories.
