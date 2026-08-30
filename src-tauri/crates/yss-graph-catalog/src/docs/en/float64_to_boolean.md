# Float64 to Boolean

Casts `DataSeries<Float64>` to `DataSeries<Boolean>`: $0 \to \text{false}$, non-zero $\to \text{true}$.

## Usage

Build boolean masks from continuous scores (e.g. probability $> 0$). NaN typically becomes null, not true/false.
