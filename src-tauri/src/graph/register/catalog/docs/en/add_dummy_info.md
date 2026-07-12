# Add Dummy Info

Attach dummy-encoding metadata to a **Categorical** **DataSeries** for **OLS** and related regression nodes. The series values are unchanged; encoding rules travel with the output reference.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataSeries** | Input | **Categorical** series to encode |
| **Drop Category** | Input (optional) | Baseline category omitted from dummies (reference level) |
| **Role** | Input (optional) | `General`, `Individual`, or `Time` — how **OLS** treats the factor |
| **DataSeries** | Output | Same categorical data with `DummyInfo` metadata attached |

## Usage

Place after extracting a categorical column (e.g. from **Decompose DataFrame**). Set **Drop Category** to the reference level Stata-style (that level gets coefficient 0). Choose **Role** when the factor is an entity ID (**Individual**) or time index (**Time**). Wire the output into **OLS** exogenous pins that accept categorical regressors.
