# TOPSIS

Default vector normalization divides each column by its Euclidean norm. Multiply by normalized weights, find benefit/cost ideal and anti-ideal values, and compute Euclidean distances $D_i^+$ and $D_i^-$. Closeness is $C_i=D_i^-/(D_i^++D_i^-)$. Weights are applied before distances (hence squared inside squared distances). Coincident ideals with zero total distance are rejected.

## Inputs and parameters

Connect **criteria** as aligned Numeric columns, one criterion per port and one alternative per row. At least one row is needed; objective weights require two or more observations. Nulls, non-finite values and different observation counts are rejected; there is no fixed row limit.

**cost_criteria** lists one-based positions where lower is better; empty means all criteria are benefits. Score normalization, when exposed, is **minmax**, **vector**, or **none**. Min–max maps constant columns to zero. For additive scores, cost criteria use one minus min–max values (constant stays zero), or negate raw/vector values. Rank and distance methods handle direction directly.

Nodes offering **weight_method** default to equal weights, except the general Weights node which defaults to entropy. Select **explicit** and connect exactly one **criterion_weights** vector to supply finite nonnegative weights in criterion-port order; length must equal the criterion count and the sum must be positive. The weight vector has its own row domain. Other methods reject a connected explicit vector. Objective weighting always uses its own preprocessing, regardless of score normalization.

## Outputs

**result** contains method, normalization, criterion names/directions, normalized weights and weighting statistics. **scores** is a pageable table containing every input row, one-based observation, score and descending rank (average ranks for ties). TOPSIS also includes distances to the ideal and anti-ideal. **weights** is a Numeric vector reusable as another node's explicit criterion weights. Higher scores indicate preferred alternatives; scores are relative to this input, not probabilities or significance tests.
