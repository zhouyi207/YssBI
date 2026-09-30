# Multiple-proportion homogeneity test

Intended to compare success probabilities across independent groups.

## Inputs and parameters

`successes_and_trials` accepts `[successes1, trials1, successes2, trials2, …]` for at least 3 groups. Values must be finite nonnegative integers; each trial count must be positive and successes cannot exceed trials. Missing values are rejected. There are no parameters.

## Hypotheses and statistic

The intended null is $H_0:p_1=\cdots=p_k$; the alternative is that at least one proportion differs. Pearson's statistic is:

$$
X^2=\sum_{ij}\frac{(O_{ij}-E_{ij})^2}{E_{ij}},\qquad
E_{ij}=\frac{O_{i+}O_{+j}}N,\qquad
X^2\overset{H_0}{\approx}\chi^2_{k-1}.
$$

$O_{ij}$ represents success/failure contingency counts and $N$ the total trial count. Valid inference requires independent trials, correct grouping, and adequate expected counts.

## Outputs and current limitation

`result` contains the structured result. `statistic` is Pearson chi-square, `degrees_of_freedom` is `[k−1]`, `p_value` is the upper chi-square tail, and `sample_sizes` contains total trials. `details.overall_proportion` is the overall success proportion; `details.minimum_expected_count` is the constructed table's minimum expected count. Estimate and standard error fields are null.

Currently, interleaved group success/failure counts are reshaped into two rows without preserving group margins correctly. The output should therefore not yet be used to infer equal group proportions. Instead, arrange one row per group with success and failure columns and use the Pearson chi-square contingency node.
