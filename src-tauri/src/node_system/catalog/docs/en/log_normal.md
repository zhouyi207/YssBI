# Log-Normal

The log-normal distribution arises when $\ln X \sim N(\mu, \sigma^2)$:

$$
f(x)=\frac{1}{x\sigma\sqrt{2\pi}}\exp\!\left(-\frac{(\ln x-\mu)^2}{2\sigma^2}\right),\quad x > 0
$$

## Usage

Set **Mu**, **Sigma**, and **N**, then run the graph. **Samples** is a positive `DataSeries<Float64>`. Use for right-skewed positive data such as income or prices, multiplicative processes, and non-negative random variables.
