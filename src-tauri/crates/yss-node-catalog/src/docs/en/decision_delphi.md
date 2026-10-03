# Delphi round summary

Connect aligned **criteria**, one rated item per column and one expert per row.
**full_score** is the scale's stated maximum (default 5); all ratings must be
finite and at most this value. Missing data are rejected.

**items** reports each item's mean, sample SD (n−1 divisor), CV = SD/|mean|,
type-7 quartiles, median, observed minimum/maximum and full-score percentage.
CV is null when the mean is zero; SD/CV are null for a single expert.
The full-score percentage uses the configured scale maximum, not the largest
rating observed in this sample.

**result** reports the expert/item counts and tie-corrected Kendall W with its
chi-squared approximation. W is null with a stated reason when fewer than two
experts/items are available or no expert distinguishes between items.
In the concordance result, observations counts items and raters counts experts.

This node summarizes one round. Expert recruitment, anonymity, feedback,
repeat rounds and substantive item-selection decisions remain researcher tasks.
