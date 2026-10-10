# Item analysis (discrimination)

Connect at least two aligned numeric **items**, with respondents on rows.
Prepare reverse-scored items first. Missing and nonfinite scores are rejected.

**tail_fraction** defaults to 0.27 and must lie strictly between 0 and 0.5.
Total-score type-7 quantiles at p and 1−p define the low and high groups.
When cutoffs differ, include boundary ties in their respective tails. When
cutoffs coincide, only scores strictly below/above enter the tails; tied cutoff
scores stay in the middle. Rows never belong to both groups.

**result** reports cutoffs, actual group counts and the shared reliability
summary. **item_statistics** includes corrected item-total correlations, alpha
if deleted, low/high means and a two-sided Welch t test (high minus low).
At least two respondents per tail and a positive standard error are required;
otherwise test fields are null. These selected-tail tests are descriptive item
screening, not independent confirmatory evidence or a causal effect.

**scores** retains every row with observation number, total score and group
(−1 low, 0 middle, 1 high). No arbitrary row limit is applied.
