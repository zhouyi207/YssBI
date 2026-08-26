# Logit

二元 Logit 回归（IRLS 估计）。对 $y_i \in \{0,1\}$：

$$
P(y_i=1 \mid x_i) = \Lambda(x_i'\beta) = \frac{1}{1+e^{-x_i'\beta}}
$$
