# Int64 to Boolean

Casts `DataSeries<Int64>` to `DataSeries<Boolean>`: $0 \to \text{false}$, non-zero $\to \text{true}$.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Int64>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | `DataSeries<Boolean>`; null → null |

## Usage

Turn numeric indicator columns into boolean masks for **Branch** or filtering. Zero is explicitly false.
