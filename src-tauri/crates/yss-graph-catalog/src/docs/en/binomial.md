# Binomial

The binomial distribution $\mathrm{Binomial}(n,p)$ counts successes in $n$ independent Bernoulli trials:

$$
P(X=k)=\binom{n}{k}p^k(1-p)^{n-k},\quad k=0,1,\ldots,n
$$

## Usage

Set **N Trials**, **P**, and **N Samples**, then run the graph. **Samples** is a `DataSeries<Int64>` where each element is the success count from one batch of $n$ trials. Common for repeated experiments, defect counts, and binomial proportion simulation.
