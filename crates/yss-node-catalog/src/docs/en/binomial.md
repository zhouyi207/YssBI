# Binomial

The binomial distribution $\mathrm{Binomial}(n,p)$ counts successes in $n$ independent Bernoulli trials:

$$
P(X=k)=\binom{n}{k}p^k(1-p)^{n-k},\quad k=0,1,\ldots,n
$$

## Usage

Set distribution parameters and sample count in the **Distribution** and **Sampling** groups in Detail. The canvas exposes the **Samples** output.

Trial Count is an integer in [0, 2^53], and success probability is in [0, 1]. Outputs Int64 success counts. Zero trials or p = 0 returns zero; p = 1 returns Trial Count.
