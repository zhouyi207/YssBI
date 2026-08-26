# Logit

Binary logistic regression estimated by IRLS. For $y_i \in \{0,1\}$:

$$
P(y_i=1 \mid x_i) = \Lambda(x_i'\beta) = \frac{1}{1+e^{-x_i'\beta}}
$$
