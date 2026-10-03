# Fuzzy comprehensive evaluation

Connect **memberships** with one grade per column and one criterion per row.
Entries are nonnegative finite memberships or counts; each row is normalized
to sum to 1 and must have a positive total.
Optional **criterion_weights** supplies one weight per row; otherwise weights
are equal. Optional **grade_scores** supplies one finite score per grade.
These two vectors may have independent row domains.

**fuzzy_operator** selects product_sum (weighted sum, default), min_max
(maximum of minima), product_max (maximum of products), or min_sum (sum of
minima bounded above by 1). The resulting grade vector is normalized.

**result** reports weights, raw and normalized memberships, all maximum-membership
grades (allowing floating-point roundoff), and the membership-weighted grade score
when grade_scores is supplied. **combined_memberships** is the normalized vector.
No ordinal grade scores are invented. Each node evaluates one criterion system;
multiple levels can be combined through their membership outputs.
