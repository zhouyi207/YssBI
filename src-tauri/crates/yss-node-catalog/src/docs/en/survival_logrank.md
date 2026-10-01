# Log-rank test

Connect aligned `time` and `event` series. Time must be finite and strictly positive, in one consistent unit; `event=1` (or true) means an observed event and `0` (or false) means right censoring. Rows with missing values are rejected, not dropped. Censoring is assumed independent of the event process, conditional on the model/group used. Equal-length columns from unrelated tables are not treated as aligned.

Connect a required `groups` label series with at least two observed groups. Labels are preserved; no parameters or covariates are used. Events precede censoring at tied times.

## Hypothesis and statistic

$H_0$: the groups have equal survival functions; $H_1$: at least one differs. At event time $t_j$, let $p_{gj}=r_{gj}/r_j$ and let $d_j$ be the total events:

$$ U_g=\sum_j(d_{gj}-d_jp_{gj}),\qquad
V_{gh}=\sum_{j:r_j>1}\frac{d_j(r_j-d_j)}{r_j-1}
(p_{gj}\mathbf1_{g=h}-p_{gj}p_{hj}).$$
Drop the last group's redundant component and compute $Q=U^\top V^{-1}U$. The asymptotic reference distribution is $\chi^2_{G-1}$ for $G$ groups. Small p-values oppose equal survival, but do not estimate an effect size or establish proportional hazards.

`result` contains original group labels, observed and expected event counts, the full covariance matrix, and `test` with statistic, degrees of freedom and p-value. No-event or singular comparisons are rejected rather than assigned p=1. All observed groups must be comparable through informative risk sets. This is the unweighted, unstratified test; crossing hazards can reduce its power.
$$
