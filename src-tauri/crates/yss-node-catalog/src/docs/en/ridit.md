# Ridit analysis

Connect an ordered **Sample** and **Reference**. Each is a Numeric category code series or an Ordinal series with an explicit level order. Samples may have different lengths and come from separate populations; they are not treated as paired observations. Both must be nonempty and complete. Ordinal inputs require the same declared code order. Numerical codes are ordered increasingly.

For reference category proportion $p_j$,

$$r_j=\sum_{h<j}p_h+\tfrac12p_j,\qquad\bar r=\sum_jq_jr_j,$$

where $q_j$ is the sample category proportion. Mean Ridit is the estimated probability that a sample observation exceeds a reference observation, with ties worth one half. Values above 0.5 indicate higher sample categories; below 0.5 indicate lower ones. The reference distribution has mean Ridit 0.5.

`result` contains category counts/proportions and scores, mean Ridit, and a tie-corrected Mann–Whitney normal test of $H_0:P(S>R)+\tfrac12P(S=R)=0.5$. **Alternative** defaults to `two_sided`; **Continuity correction** defaults to false. For small samples the normal approximation can be poor; this node does not compute an exact rank-test p-value. With zero null variance, inference fields are null while the descriptive scores remain available.

Reference: [Bross (1958), How to Use Ridit Analysis](https://doi.org/10.2307/2527727).
