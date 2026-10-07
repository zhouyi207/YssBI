# Geometric

The geometric distribution $\mathrm{Geometric}(p)$ models the number of trials until the first success (including the successful trial):

$$
P(X=k)=(1-p)^{k-1}p,\quad k=1,2,3,\ldots
$$

## Usage

Set distribution parameters and sample count in the **Distribution** and **Sampling** groups in Detail. The canvas exposes the **Samples** output.

Require 0 < p ≤ 1; p = 1 returns 1. Samples count the successful trial too. A sample exceeding the exact count limit of 2^53 fails the operation.
