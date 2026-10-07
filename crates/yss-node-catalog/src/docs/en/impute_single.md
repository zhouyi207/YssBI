# Single imputation

Connect a Numeric **series**. The **result** is a Numeric series with the same rows,
order and row domain. Only null cells are replaced. Non-finite numbers are rejected.

Choose **imputation_method**:

- **mean** (default): mean of all observed values.
- **median**: exact median of all observed values; even samples use the middle-pair average.
- **mode**: most frequent numeric value; ties use the smallest value.
- **constant**: the finite **fill_value**, default 0.

The first three methods require at least one observed value in a nonempty series.
An all-null series fails for those methods; constant imputation can fill it.
An empty input remains empty. No fixed row limit is imposed. The operation is a
native relation plan and does not materialize the whole column in the node adapter.

Output values use floating-point storage. Single imputation does not account for
uncertainty in missing values and can distort variances, correlations and tests.
Use training-only estimates for predictive validation: this node estimates the
fill value from its entire input, without knowledge of training/test partitions.
Forward/backward filling is available through its dedicated ordered-series nodes.
