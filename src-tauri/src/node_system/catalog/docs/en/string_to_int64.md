# String to Int64

Parses a `DataSeries<String>` to `DataSeries<Int64>` element-wise via Polars cast.

## Usage

Convert ID or count columns stored as text. Fractional strings fail parse and become null.
