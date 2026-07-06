# Bernoulli

The Bernoulli distribution $\mathrm{Bernoulli}(p)$ models a single trial with outcomes 1 (success) or 0 (failure):

$$
P(X=1)=p,\quad P(X=0)=1-p
$$

Draw **N** independent samples from $\mathrm{Bernoulli}(p)$.

## Pins

| Pin | Description |
|-----|-------------|
| **P** | success probability $p$, with $0 \le p \le 1$ |
| **N** | sample size (non-negative integer) |

## Usage

Wire **P** and **N**, then run the graph. **Samples** is a `DataSeries<Int64>` of length **N** containing 0/1 values. Use for Monte Carlo simulation, binary outcome generation, or as building blocks alongside **Binomial** and other random nodes.
