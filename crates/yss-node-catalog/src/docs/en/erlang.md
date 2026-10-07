# Erlang

The Erlang distribution is a special case of **Gamma** with integer shape **K**: the sum of **K** independent exponential waiting times with rate $\lambda$:

$$
X=\sum_{i=1}^{K} \mathrm{Exp}(\lambda)
$$

## Usage

Set distribution parameters and sample count in the **Distribution** and **Sampling** groups in Detail. The canvas exposes the **Samples** output.

Integer Shape is in [1, 2^53], and Rate must be finite and positive. Uses the Gamma shape/rate convention and outputs positive Float64 samples.
