# NRI and IDI

Connect aligned **outcome** (event 1/true, nonevent 0/false), **reference** event probabilities and **new** event probabilities. Both outcome groups must occur, probabilities must lie in $[0,1]$, and missing rows are rejected.

**Reclassification mode** defaults to continuous: increases in probability count as upward movement, decreases as downward movement, and equal probabilities as unchanged. Leave **Risk thresholds** empty. Categorical mode requires strictly increasing cut points within $(0,1)$ and compares risk categories instead; equality with a cut point enters the higher category.

$$
NRI=[P(up\mid Y=1)-P(down\mid Y=1)]+[P(down\mid Y=0)-P(up\mid Y=0)].
$$

$$
IDI=(\bar p_{new,1}-\bar p_{new,0})-(\bar p_{ref,1}-\bar p_{ref,0}).
$$

Subscripts 1/0 denote events/nonevents. IDI always uses original probabilities, independently of risk grouping.

**result** reports both groups' sizes and upward/downward/unchanged counts, nri_events, nri_nonevents, nri, both discrimination slopes and idi. Positive values indicate improvement on the corresponding measure. Outputs are point estimates, without confidence intervals or significance tests; censored outcomes are unsupported. Predictions must refer to the same validation subjects, event definition and prediction horizon.
