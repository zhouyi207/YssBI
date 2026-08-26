# Int64 to Boolean

Casts `DataSeries<Int64>` to `DataSeries<Boolean>`: $0 \to \text{false}$, non-zero $\to \text{true}$.

## Usage

Turn numeric indicator columns into boolean masks for **Branch** or filtering. Zero is explicitly false.
