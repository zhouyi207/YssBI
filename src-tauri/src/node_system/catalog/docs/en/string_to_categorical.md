# String to Categorical

Casts a `DataSeries<String>` to `DataSeries<Categorical>` using Polars categorical encoding.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<String>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Output `DataSeries<Categorical>` with category pool built from distinct strings |

## Usage

Turn free-text codes into factor-like columns for **Logit**, **Probit**, or panel regressors. Null strings remain null.
