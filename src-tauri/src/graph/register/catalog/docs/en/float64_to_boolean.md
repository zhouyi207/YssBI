# Float64 to Boolean

Casts `DataSeries<Float64>` to `DataSeries<Boolean>`: $0 \to \text{false}$, non-zero $\to \text{true}$.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Float64>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | `DataSeries<Boolean>`; null / non-finite per Polars rules |

## Usage

Build boolean masks from continuous scores (e.g. probability $> 0$). NaN typically becomes null, not true/false.
