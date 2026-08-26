# TS Pct Change

Computes percentage change:

$$
\frac{y_t - y_{t-\text{lag}}}{y_{t-\text{lag}}}
$$

First **lag** observations and zero denominators yield null.

## Usage

Use for returns or growth rates. Uses positional lag without a time column; for strict calendar semantics, use **TS Diff** and divide manually.
