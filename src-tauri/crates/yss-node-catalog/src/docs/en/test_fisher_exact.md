# Fisher exact test

Performs a two-sided conditional exact test of independence for two binary classifications.

## Inputs and parameters

`row` and `column` must be nonempty, aligned categorical series of equal length, each with exactly 2 observed levels. Numeric, Categorical, Ordinal, and Binary semantics are supported. Relational series must share a row domain. Missing values are rejected. There are no parameters; only a two-sided test is available.

## Hypotheses and reference distribution

$H_0$: the variables are independent, with population odds ratio 1. $H_1$: they are associated. Write the observed table as $\begin{pmatrix}a&b\\c&d\end{pmatrix}$ and set $r_1=a+b$, $r_2=c+d$, $c_1=a+c$, $N=r_1+r_2$. Conditional on the margins:

$$
P(A=x)=\frac{\binom{r_1}{x}\binom{r_2}{c_1-x}}{\binom N{c_1}}.
$$

The two-sided p-value sums all feasible tables whose probability does not exceed that of the observed table. No chi-square approximation or reference degrees of freedom are used. Observations must still be independent.

## Outputs and interpretation

`result` and `report` contain the same report. `p_value` is the two-sided conditional exact p-value. `details.a`, `b`, `c`, and `d` contain the table formed in category-code order; `sample_sizes` is `[N]`. `estimate` and `standard_error` are null and degrees of freedom are empty.

Currently `statistic_name` is `odds_ratio`, but `statistic` stores $ad-bc$, not an odds ratio. Interpret independence through `p_value`; reject $H_0$ when $p<\alpha$.
