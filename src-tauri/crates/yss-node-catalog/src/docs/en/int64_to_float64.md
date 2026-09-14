# Int64 to Float64

Casts `DataSeries<Int64>` to `DataSeries<Float64>` with exact integer representation where possible.

## Usage

Prepare integer columns for **Ln**, **Divide**, or **OLS** without losing integer values within `f64` precision. Null stays null.
