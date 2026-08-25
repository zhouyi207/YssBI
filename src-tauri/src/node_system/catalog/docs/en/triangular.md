# Triangular

The triangular distribution is defined by minimum **A**, maximum **B**, and mode **C** ($A \le C \le B$), with peak density at **C**:

$$
f(x)=\begin{cases}
\frac{2(x-A)}{(B-A)(C-A)} & A \le x < C \\
\frac{2(B-x)}{(B-A)(B-C)} & C \le x \le B
\end{cases}
$$

## Pins

| Pin | Description |
|-----|-------------|
| **A** | lower bound (minimum) |
| **B** | upper bound (maximum), with $A \le B$ |
| **C** | mode, with $A \le C \le B$ |
| **N** | sample size |

## Usage

Set **A**, **B**, **C**, and **N**, then run the graph. **Samples** is a `DataSeries<Float64>` on $[A,B]$. Use for expert elicitation, project duration uncertainty, and bounded but non-uniform random inputs.
