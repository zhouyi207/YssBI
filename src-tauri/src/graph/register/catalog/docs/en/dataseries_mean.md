# DataSeries Mean

Computes the arithmetic mean of a numeric **DataSeries** and returns a **Float64** scalar.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataSeries** | Input | Numeric series |
| **Mean** | Output | Sample mean (`Float64`) |

## Usage

Connect **Get DataSeries** or any numeric series. The node errors if the mean cannot be computed (e.g. all nulls).
