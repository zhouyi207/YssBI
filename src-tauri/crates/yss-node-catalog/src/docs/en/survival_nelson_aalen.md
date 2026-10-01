# Nelson–Aalen cumulative hazard

Connect aligned `time` and `event` series. Time must be finite and strictly positive, in one consistent unit; `event=1` (or true) means an observed event and `0` (or false) means right censoring. Rows with missing values are rejected, not dropped. Censoring is assumed independent of the event process, conditional on the model/group used. Equal-length columns from unrelated tables are not treated as aligned.

Optional `groups` accepts at most one exact label series; otherwise all rows form one group. No parameters are required. Events precede censors at tied times.

## Estimator and interpretation

With $d_j$ events among $r_j$ at risk:

$$ \widehat H(t)=\sum_{t_j\le t}d_j/r_j,\qquad
\widehat{\mathrm{Var}}\{\widehat H(t)\}=\sum_{t_j\le t}d_j/r_j^2.$$
This uses the counting-process/Poisson variance convention. `result.curves` contains group and sample/event counts, median, and time points with risk/event/censor counts. `estimate` and `cumulative_hazard` equal $\widehat H$; `standard_error` is its standard error. The reported survival is $\exp(-\widehat H)$, not Kaplan–Meier.

The 95% hazard interval is $\widehat H\exp\{\pm1.96\,SE/\widehat H\}$, with [0,0] when hazard is zero. Median is the first time this derived survival is at most 0.5; it can remain null even after the last observed event. All-censored data return zero hazard. There is no hypothesis test or p-value. Independent subjects and right censoring only; no delayed entry, interval censoring or weighting.
$$
