# Hypergeometric

The hypergeometric distribution models the number of successes when drawing $n$ items **without replacement** from a finite population of size **N** containing **K** successes:

$$
P(X=k)=\frac{\binom{K}{k}\binom{N-K}{n-k}}{\binom{N}{n}}
$$

## Usage

Set distribution parameters and sample count in the **Distribution** and **Sampling** groups in Detail. The canvas exposes the **Samples** output.

Population Size, Successes in Population and Draw Count are non-negative integers; successes and draws cannot exceed the population. Zero draws return zero and drawing the whole population returns its success count. Each output is an independent experiment from the same population. Sampling uses exact integers and checks cancellation within long draw loops.
