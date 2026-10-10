# Normal

The normal (Gaussian) distribution $N(\mu, \sigma^2)$ has density:

$$
f(x)=\frac{1}{\sqrt{2\pi\sigma^2}}\exp\!\left(-\frac{(x-\mu)^2}{2\sigma^2}\right)
$$

## Usage

Set distribution parameters and sample count in the **Distribution** and **Sampling** groups in Detail. The canvas exposes the **Samples** output.

Mean must be finite; Standard Deviation must be finite and strictly positive. Produces Float64 samples. New samples are generated each execution; results are not cached.
