# Uniform

The continuous uniform distribution $\mathrm{Uniform}(a,b)$ has constant density on $[a,b)$:

$$
f(x)=\frac{1}{b-a},\quad a \le x < b
$$

## Usage

Set distribution parameters and sample count in the **Distribution** and **Sampling** groups in Detail. The canvas exposes the **Samples** output.

Lower Bound and Upper Bound must be finite, with lower < upper and a finite width. Samples belong to [lower, upper); the upper bound is excluded.
