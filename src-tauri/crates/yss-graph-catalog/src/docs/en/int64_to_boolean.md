# Int64 to Boolean

Casts `DataSeries<Int64>` to `DataSeries<Boolean>`: $0 \to \text{false}$, non-zero $\to \text{true}$.

## Usage

Turn numeric indicator columns into Boolean masks for filtering or downstream transformations. Zero is explicitly false.
