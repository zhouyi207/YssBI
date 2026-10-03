# Reliability (Cronbach alpha)

Connect at least two aligned numeric **items**; rows are respondents, with at
least two complete respondents. Missing and nonfinite values are rejected.
Reverse-score items and choose a single intended scale before connecting them;
the node does not reverse items automatically.

**result** reports raw and standardized Cronbach alpha, total-score mean and
sample standard deviation. Raw alpha uses the sum-score variance and individual
item variances. Negative alpha values are retained. Standardized alpha uses
unit-variance items and is null if any item is constant.

**item_statistics** includes item mean, sample standard deviation, corrected
item-total correlation (total excluding that item), and raw alpha after deleting
the item. Zero-variance correlations and alpha with fewer than two remaining
items or zero total variance are null.

Alpha assesses internal consistency under its measurement assumptions; it does
not establish unidimensionality or validity. This node does not estimate omega
or test-retest reliability. All observations are used, subject to execution
memory, cancellation and time budgets.
