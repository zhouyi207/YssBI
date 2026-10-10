# Poisson

The Poisson distribution $\mathrm{Poisson}(\lambda)$ models the count of rare events in a fixed interval:

$$
P(X=k)=\frac{e^{-\lambda}\lambda^k}{k!},\quad k=0,1,2,\ldots
$$

## Usage

Set distribution parameters and sample count in the **Distribution** and **Sampling** groups in Detail. The canvas exposes the **Samples** output.

Rate must be finite and within [0, 2^53]; a zero rate produces zeros. Outputs non-negative Int64 samples. Samples exceeding the exact count limit of 2^53 fail the operation.
