# Geometric

The geometric distribution $\mathrm{Geometric}(p)$ models the number of trials until the first success (including the successful trial):

$$
P(X=k)=(1-p)^{k-1}p,\quad k=1,2,3,\ldots
$$

## Usage

Wire **P** and **N**, then run the graph. **Samples** is a `DataSeries<Int64>` of positive integers. Use for time-to-first-success, waiting counts in repeated trials, and related survival-style discrete models.
