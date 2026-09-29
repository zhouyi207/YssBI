# McNemar test

Tests whether success probabilities are equal across two measurements on the same subjects.

## Inputs and parameters

Connect numeric `0/1` series to `before` and `after` in matching subject order. They must be nonempty and equal in length. `1` must retain the same meaning across measurements; handle missing values jointly beforehand. There are no parameters.

At least one discordant pair, `0→1` or `1→0`, is required. If all pairs agree, no test result is returned.

## Hypotheses and statistic

$H_0$: the marginal success probabilities are equal, so both discordant directions are equally likely. $H_1$: marginal probabilities differ. Let $b$ count `0→1` pairs, $c$ count `1→0` pairs, and $D=b+c$:

$$
X^2=\frac{(b-c)^2}{b+c},\qquad B\mid D\overset{H_0}{\sim}\operatorname{Binomial}(D,1/2),
$$

$$
p=\min\{1,\;2P(B\le\min(b,c))\}.
$$

The displayed $X^2$ has an asymptotic $\chi^2_1$ reference, but the reported p-value uses the exact binomial calculation above. Subjects must be independent; measurements within a subject remain paired.

## Outputs and interpretation

`result` and `report` are identical. `statistic` is $X^2$ and the degrees-of-freedom field is `[1]`. `p_value` and `details.exact_binomial_p_value` are exact p-values. `details.discordant_0_to_1` and `details.discordant_1_to_0` give $b,c$; `sample_sizes` contains the total pair count. Estimate and standard error are null.

Reject equal marginal probabilities when $p<\alpha$. Only discordant pairs contribute to this test.
