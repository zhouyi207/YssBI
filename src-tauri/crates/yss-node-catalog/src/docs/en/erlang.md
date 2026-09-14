# Erlang

The Erlang distribution is a special case of **Gamma** with integer shape **K**: the sum of **K** independent exponential waiting times with rate $\lambda$:

$$
X=\sum_{i=1}^{K} \mathrm{Exp}(\lambda)
$$

## Usage

Set distribution parameters and sample count in **Detail → Configuration**. The canvas exposes the **Samples** output.

Set **K**, **Rate**, and **N**, then run the graph. **Samples** is a non-negative `DataSeries<Float64>`. Use for total waiting time in $K$-stage queues, telephony systems, and service-process modeling.
