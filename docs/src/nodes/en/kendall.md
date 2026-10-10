# Kendall rank correlation

Connect aligned Numeric or Ordinal **X** and **Y**. At least two complete observations are needed. Ordinal data uses its declared level order. This node computes tau-b, correcting for ties in both variables:

$$\tau_b=\frac{C-D}{\sqrt{(n_0-T_X)(n_0-T_Y)}},\quad n_0=n(n-1)/2.$$

C and D are concordant and discordant pairs. $T_X,T_Y$ include all tied pairs in each variable, including joint ties. A constant variable makes the denominator zero and fails.

Under the null of no ordinal association, choose `two_sided` (default), `greater`, or `less`. `auto` uses exact position permutations for at most nine observations, including ties, and otherwise a tie-corrected normal approximation to $C-D$. `permutation_exact` is limited to nine; `asymptotic` requires at least three observations. Exact two-sided p-values use twice the smaller inclusive tail, capped at 1.

`result` contains tau-b, sample size, pair/tie counts, and inference. Unlike the conventional no-tie exact distribution, the exact mode here conditions on the observed ranks and enumerates position permutations. No confidence interval is supplied.

References: [SciPy tau-b](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.kendalltau.html), [permutation tests](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.permutation_test.html).
