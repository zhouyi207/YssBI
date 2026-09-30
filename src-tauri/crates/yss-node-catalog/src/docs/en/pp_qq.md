# P-P / Q-Q Plot

Values takes at least two finite Numeric observations. The reference is normal. Defaults to Q-Q with the sample mean and sample standard deviation estimated from the input. Disable estimation to use Reference mean (default 0) and Reference standard deviation (default 1, positive).

Sorted observation $i$ uses $p_i=(i-0.5)/n$, $i=1,\ldots,n$. Q-Q compares $F^{-1}(p_i)$ with observed quantiles; P-P compares $F(x_{(i)})$ with $p_i$. The reference line is $y=x$. Constant samples cannot estimate a positive reference standard deviation. At most 2048 points are drawn.

Open the result output in a workbench result panel or a separate Plot window.
