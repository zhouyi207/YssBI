# Confidence intervals

Connect aligned numeric estimates and standard_errors. Values must be finite, SEs nonnegative and the table nonempty. Each row is a supplied estimate with its already calculated SE; this node does not estimate sampling variance from raw observations.

confidence_level defaults to 0.95 and must be strictly between 0 and 1. degrees_of_freedom defaults to 0, selecting the normal reference; a positive finite value selects Student t with that many degrees of freedom for all rows.

For confidence level $c$, the interval is $\hat\theta\pm q_{(1+c)/2}s$, where $s$ is the supplied SE and $q$ is the standard-normal or t quantile. A zero SE gives a point interval. Interpretation depends on the validity of the supplied SE and reference approximation. These intervals are on the estimate's original scale; they are not clipped to probability limits, multiplicity-adjusted or transformed.

result reports rows, confidence_level, reference_distribution and degrees_of_freedom (null for normal). The intervals table contains index (one-based input row), estimate, standard_error, lower and upper. It can be paged or connected to downstream table nodes. No P value or new hypothesis test is inferred from these inputs.
