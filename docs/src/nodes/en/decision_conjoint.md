# Ratings-based conjoint

Connect numeric **ratings** and aligned categorical **factors**. Each row is
a rated profile. Numeric factor codes are treated as categories, not slopes.
Labels retain their exact values, ordered by first appearance.
Missing/nonfinite ratings and missing categories are rejected.

The additive OLS design includes an intercept and L−1 indicators per attribute.
It must have full rank; the sample needs at least as many rows as parameters.
Each attribute's part-worth utilities are centered to sum to zero and the
intercept is adjusted accordingly. Importance is its utility range divided by
the sum of attribute ranges; all importance values are null if all ranges are zero.

**result** reports level utilities, conventional independent homoscedastic OLS
standard errors, importance percentages, the centered intercept, R² and residual
degrees of freedom. Without residual degrees of freedom standard errors are null;
R² is null for constant ratings.
**predictions** retains all observations with fitted ratings and residuals.

This is an additive ratings model. It does not estimate choice probabilities,
attribute interactions or respondent-specific effects. Repeated respondents
require an appropriate dependence model before interpreting standard errors.
