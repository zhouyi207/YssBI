# Negative Binomial

The negative binomial distribution $\mathrm{NB}(r,p)$ counts failures before the $r$-th success:

$$
P(X=k)=\binom{k+r-1}{k}(1-p)^k p^r,\quad k=0,1,2,\ldots
$$

## Usage

Set distribution parameters and sample count in **Detail → Configuration**. The canvas exposes the **Samples** output.

Success Count is a positive real shape r; success probability must satisfy 0 < p ≤ 1. Outputs Int64 failure counts, excluding successes; p = 1 produces zeros. Samples exceeding the exact count limit of 2^53 fail the operation.
