# Competing risks: Aalen–Johansen

Connect aligned positive finite `time` and numeric integer `status` series. Status 0 means censoring; each distinct positive integer identifies an event cause. At least one event is required. Codes need not be consecutive and are preserved in ascending order. Missing values and fractional/negative statuses are rejected. No parameters, predictors or grouping are used.

## Estimator

Let $d_{kj}$ be cause-$k$ events at $t_j$, $r_j$ the risk set, and $S(t_j-)$ event-free survival immediately before that time:

$$ \widehat F_k(t)=\sum_{t_j\le t}\widehat S(t_j-)\frac{d_{kj}}{r_j},\qquad
\widehat S(t_j)=\widehat S(t_j-)\left(1-\frac{\sum_kd_{kj}}{r_j}\right).$$
Events of all causes precede tied censors. An event from another cause is **not** treated as independent censoring for this cumulative incidence calculation.

`result.causes` maps every events/incidence vector position to its original code. `points` includes all observed times, at-risk and censor counts, event counts by cause, event-free survival and cumulative incidence. Survival plus the sum of cumulative incidences is one. This is a nonparametric point estimator, not Fine–Gray or cause-specific Cox regression; no regression effects, standard errors, confidence intervals or p-values are supplied. Assumes independent subjects and independent right censoring; no delayed entry or recurrent events.
$$
