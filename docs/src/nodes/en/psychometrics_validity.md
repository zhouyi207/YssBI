# Validity screening (KMO / Bartlett)

Connect at least two aligned numeric **items**, with respondents on rows.
The number of complete observations must exceed the number of items and their
correlation matrix must be nonsingular. Constant items, missing and nonfinite
values are rejected.

**result** reports overall KMO, each item's measure of sampling adequacy
(`item_msa`, matching `item_names`), and Bartlett's sphericity chi-square,
degrees of freedom and approximate p-value. KMO compares squared correlations
with squared partial correlations; a zero denominator produces null.

These statistics assess whether the correlation structure supports factor
analysis. They do not prove content, convergent or discriminant validity.
Use the exploratory factor analysis node for loadings, factor scores and
communalities. Both nodes share the same KMO/Bartlett implementation.
Every observation is used, within execution memory, cancellation and time budgets.
