# Entropy weights

First min–max scale each criterion and reverse cost criteria. For proportions $p_{ij}=z_{ij}/\sum_i z_{ij}$, entropy is $e_j=-\sum_i p_{ij}\ln(p_{ij})/\ln(n)$ and weights are proportional to $1-e_j$. Zero-probability terms contribute zero. Constant columns have zero information. All-zero information is rejected.

## Inputs and parameters

Connect **criteria** as aligned Numeric columns, one criterion per port and one alternative per row. At least one row is needed; objective weights require two or more observations. Nulls, non-finite values and mismatched row domains are rejected; there is no fixed row limit.

**cost_criteria** lists one-based positions where lower is better; empty means all criteria are benefits. Score normalization, when exposed, is **minmax**, **vector**, or **none**. Min–max maps constant columns to zero. For additive scores, cost criteria use one minus min–max values (constant stays zero), or negate raw/vector values. Rank and distance methods handle direction directly.

Nodes offering **weight_method** default to equal weights, except the general Weights node which defaults to entropy. Select **explicit** and connect exactly one **criterion_weights** vector to supply finite nonnegative weights in criterion-port order; length must equal the criterion count and the sum must be positive. The weight vector has its own row domain. Other methods reject a connected explicit vector. Objective weighting always uses its own preprocessing, regardless of score normalization.

## Outputs

**result** contains method, normalization, criterion names/directions, normalized weights and weighting statistics. **scores** is a pageable table containing every input row, one-based observation, score and descending rank (average ranks for ties). TOPSIS also includes distances to the ideal and anti-ideal. **weights** is a Numeric vector reusable as another node's explicit criterion weights. Higher scores indicate preferred alternatives; scores are relative to this input, not probabilities or significance tests.
