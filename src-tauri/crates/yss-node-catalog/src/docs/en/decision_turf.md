# TURF combination search

Connect aligned numeric **criteria**, one option per column and respondent per row.
Values must be 0 (not reached/interested) or 1 (reached/interested).
Missing and nonfinite values are rejected. Set **combination_size** to the number
of options to select, between 1 and the number of connected columns.

All subsets of that size are evaluated for maximum unduplicated reach.
**result** reports the selected one-based criterion positions, their names,
respondents reached, reach percentage, total exposures within the selected set,
and exposures per reached respondent (null for zero reach).
It also reports evaluated subsets and the number sharing the maximum reach.
Ties select the lexicographically first combination in input order.

This is exact search with packed observation sets. Large searches remain subject
to execution time and memory budgets; interruption produces an error, not a
partial optimum. No option or observation count cap is imposed.
