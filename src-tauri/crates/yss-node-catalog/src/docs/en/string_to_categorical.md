# String to Categorical

Casts a `DataSeries<String>` to `DataSeries<Categorical>` using Polars categorical encoding.

## Usage

Turn free-text codes into factor-like columns for **Logit**, **Probit**, or panel regressors. Null strings remain null.
