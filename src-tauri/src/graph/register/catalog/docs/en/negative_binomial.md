# Negative Binomial

The negative binomial distribution $\mathrm{NB}(r,p)$ counts failures before the $r$-th success:

$$
P(X=k)=\binom{k+r-1}{k}(1-p)^k p^r,\quad k=0,1,2,\ldots
$$

## Pins

| Pin | Description |
|-----|-------------|
| **R** | target number of successes $r > 0$ |
| **P** | success probability $p$ per trial |
| **N** | sample size |

## Usage

Set **R**, **P**, and **N**, then run the graph. **Samples** is a `DataSeries<Int64>` of non-negative integers. Use for overdispersed count data and modeling failures before a fixed number of successes.
