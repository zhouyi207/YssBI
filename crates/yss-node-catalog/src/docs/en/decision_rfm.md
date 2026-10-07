# RFM customer scoring

Connect three aligned, already aggregated customer columns:
**recency** = days since last purchase (nonnegative),
**frequency** = purchase count (nonnegative integer),
**monetary** = net spend (finite, negative refunds permitted).
Each row represents one customer. Aggregate transactions before using this node.
Missing and nonfinite values are rejected.

Each column uses average ranks. Recency is reversed so fewer days score higher.
For a desirability rank r among n customers, the score is
floor(5 × (r − 0.5)/n) + 1, giving tiers 1–5.
Identical values always share a tier; a constant column receives tier 3.
Small or tied datasets need not occupy all five tiers.

**result** describes the scoring convention and tier frequencies.
**scores** contains observation position, the three tier scores and their sum
(3–15), in original row order. These are relative sample scores, without
automatic marketing segment labels.
