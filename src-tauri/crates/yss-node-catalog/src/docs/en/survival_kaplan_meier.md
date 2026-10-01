# Kaplan–Meier survival

Connect aligned `time` and `event` series. Time must be finite and strictly positive, in one consistent unit; `event=1` (or true) means an observed event and `0` (or false) means right censoring. Rows with missing values are rejected, not dropped. Censoring is assumed independent of the event process, conditional on the model/group used. Equal-length columns from unrelated tables are not treated as aligned.

An optional `groups` input accepts one label series; omitted means a single curve. Text, categorical, ordinal, binary and exact numeric labels are retained. There are no parameters. Event times need not be pre-sorted.

## Estimator and output

At distinct observed time $t_j$, let $r_j$ be the number still at risk and $d_j$ the number of events. Events are processed before censoring at the same time:
$$\widehat S(t)=\prod_{t_j\le t}(1-d_j/r_j).$$
`result.curves` contains each group, observation/event counts, median survival and all observed-time points, including censor-only times. Each point records time, at-risk count, events, censored count, survival, estimate, standard error and a 95% interval. `estimate` equals survival; `cumulative_hazard` is the Nelson–Aalen sum, not minus log survival.

Greenwood variance uses $\widehat S(t)^2\sum d_j/[r_j(r_j-d_j)]$; intervals use the log-log transform. Survival 1 or 0 has the degenerate interval [1,1] or [0,0]. Median is the first observed time with survival at most 0.5, otherwise null. All-censored data yield survival 1 and no median. No group-comparison p-value is computed; connect Log-rank for that purpose.

Independent subjects and right censoring only; no entry times, interval censoring or weights. See the [survival manual](https://therneau.r-universe.dev/survival/doc/manual.html).
