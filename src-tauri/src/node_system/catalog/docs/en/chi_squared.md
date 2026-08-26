# Chi-Squared

The chi-squared distribution $\chi^2(\nu)$ is the sum of squares of $\nu$ independent standard normal variables:

$$
f(x)=\frac{x^{\nu/2-1}e^{-x/2}}{2^{\nu/2}\Gamma(\nu/2)},\quad x > 0
$$

## Usage

Wire **DF** and **N**, then run the graph. **Samples** is a non-negative `DataSeries<Float64>`. Use for variance tests, goodness-of-fit statistics, and as a building block of the **FisherSnedecor** distribution.
