# Content validity (expert CVI)

Connect numeric **items**, one column per item and one row per expert. All
columns must align. Relevance ratings must be integers 1, 2, 3 or 4; scores 3
and 4 count as relevant. Other scales need an explicit prior recoding.
Missing and nonfinite ratings are rejected.

For each item, **item_statistics** reports the number A of relevant ratings,
I-CVI = A/N, chance agreement Pc = Binomial(N, 0.5).pmf(A), and modified
kappa = (I-CVI − Pc)/(1 − Pc). The chance calculation remains stable for
large expert panels without factorial overflow.

**result** reports the average I-CVI (S-CVI/Ave) and the fraction of items
unanimously considered relevant (S-CVI/UA). These are different summaries;
the node does not turn either into an automatic pass/fail judgment.
Expert relevance evidence does not establish the scale's internal structure.
All experts and items are used within execution resource budgets.
