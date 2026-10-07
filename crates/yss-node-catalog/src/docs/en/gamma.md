# Gamma

The gamma distribution $\mathrm{Gamma}(\alpha, \beta)$ in shape–rate form has density:

$$
f(x)=\frac{\beta^\alpha}{\Gamma(\alpha)}x^{\alpha-1}e^{-\beta x},\quad x > 0
$$

## Usage

Set distribution parameters and sample count in the **Distribution** and **Sampling** groups in Detail. The canvas exposes the **Samples** output.

Shape and Rate must be finite and strictly positive. Rate is the inverse of scale: the mean is Shape / Rate. Produces positive Float64 samples.
