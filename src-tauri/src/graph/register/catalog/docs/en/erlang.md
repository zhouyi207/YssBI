# Erlang

The Erlang distribution is a special case of **Gamma** with integer shape **K**: the sum of **K** independent exponential waiting times with rate $\lambda$:

$$
X=\sum_{i=1}^{K} \mathrm{Exp}(\lambda)
$$

## Pins

| Pin | Description |
|-----|-------------|
| **K** | integer shape parameter, with $K \ge 1$ |
| **Rate** | rate parameter $\lambda > 0$ |
| **N** | sample size |

## Usage

Set **K**, **Rate**, and **N**, then run the graph. **Samples** is a non-negative `DataSeries<Float64>`. Use for total waiting time in $K$-stage queues, telephony systems, and service-process modeling.
