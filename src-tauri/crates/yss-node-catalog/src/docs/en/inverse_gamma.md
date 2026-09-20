# Inverse Gamma

The inverse gamma distribution $\mathrm{InvGamma}(\alpha, \beta)$ is the reciprocal of a gamma variable and is common as a variance prior:

$$
f(x)=\frac{\beta^\alpha}{\Gamma(\alpha)}x^{-\alpha-1}e^{-\beta/x},\quad x > 0
$$

## Usage

Set distribution parameters and sample count in **Detail → Configuration**. The canvas exposes the **Samples** output.

Shape and Scale must be finite and strictly positive. Scale is beta in exp(-beta / x): sample the reciprocal of Gamma(shape, rate = scale). Produces positive Float64 samples.
