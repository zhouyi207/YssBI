# Discrete Uniform

The discrete uniform distribution assigns equal probability to each integer in $[\mathrm{Low}, \mathrm{High}]$:

$$
P(X=k)=\frac{1}{\mathrm{High}-\mathrm{Low}+1},\quad k=\mathrm{Low},\ldots,\mathrm{High}
$$

## Pins

| Pin | Description |
|-----|-------------|
| **Low** | lower bound (inclusive) |
| **High** | upper bound (inclusive), with $\mathrm{Low} \le \mathrm{High}$ |
| **N** | sample size |

## Usage

Set **Low**, **High**, and **N**, then run the graph. **Samples** is a `DataSeries<Int64>`. Use for fair random integers, dice-style simulation, and uniform discrete sampling baselines.
