# Measurement system (crossed Gage R&R)

Connect aligned **measurements**, categorical **parts** and **operators** in
long format. At least two parts and two operators are required; every part ×
operator cell must have the same number of repetitions, at least two. Missing,
nonfinite, incomplete or unbalanced designs are rejected. Original labels are
retained in the result. No maximum number of measurements is imposed.

**include_interaction** defaults to true. The balanced crossed random-effects
ANOVA estimates repeatability, operator, part × operator and part variance.
Part and operator F tests use the interaction mean square as denominator;
the interaction test uses repeatability. With interaction disabled, its sum of
squares and degrees of freedom are pooled into repeatability; no automatic
significance-based model selection is performed.

**result** includes the ANOVA table and variance components. Reproducibility
combines operator and interaction; total Gage R&R adds repeatability; total
variation also includes part variance. Negative component estimates are set to
zero and listed explicitly. Study variation is six standard deviations.
Variance contribution percentages and study-variation percentages differ and
must not be interchanged. Zero total variation yields null percentages; a zero
test denominator yields null F/p values.

This evaluates continuous, balanced crossed measurements. Nested designs,
attribute agreement, reference-value bias, linearity and stability require
different studies. The node does not issue an automatic acceptance verdict.
