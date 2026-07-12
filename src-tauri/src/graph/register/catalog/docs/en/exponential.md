# Exponential

The exponential distribution $\mathrm{Exp}(\lambda)$ models memoryless waiting times with rate $\lambda$:

$$
f(x)=\lambda e^{-\lambda x},\quad x \ge 0
$$

Mean $\mathbb{E}[X]=1/\lambda$ and variance $\mathrm{Var}(X)=1/\lambda^2$.

## Pins

| Pin | Description |
|-----|-------------|
| **Rate** | rate parameter $\lambda > 0$ |
| **N** | sample size |

## Usage

Wire **Rate** and **N**, then run the graph. **Samples** is a non-negative `DataSeries<Float64>`. Use for survival analysis, inter-arrival times, and event gaps in Poisson processes.
