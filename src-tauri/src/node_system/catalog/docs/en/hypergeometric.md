# Hypergeometric

The hypergeometric distribution models the number of successes when drawing $n$ items **without replacement** from a finite population of size **N** containing **K** successes:

$$
P(X=k)=\frac{\binom{K}{k}\binom{N-K}{n-k}}{\binom{N}{n}}
$$

## Pins

| Pin | Description |
|-----|-------------|
| **N** | population size |
| **K** | number of successes in the population, with $0 \le K \le N$ |
| **n** | draw size per sample, with $0 \le n \le N$ |
| **N Samples** | number of independent draws |

## Usage

Set **N**, **K**, **n**, and **N Samples**, then run the graph. **Samples** is a `DataSeries<Int64>` where each element is the success count from one without-replacement draw. Use for QC sampling, finite-population proportion studies, and comparisons with the binomial model.
