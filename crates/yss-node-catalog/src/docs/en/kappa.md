# Kappa agreement

Each **Rater** input is an aligned classification column over the same subjects. Add raters in Details. Numeric, categorical, ordinal, binary, text, and identifier codes are supported; missing ratings fail. Codes retain their original values, including wide integer labels.

**Kappa definition** defaults to `cohen`, which requires exactly two raters. Choose `fleiss` for interchangeable raters with the same complete number of ratings per subject. Cohen uses each rater's category margins; Fleiss uses pooled margins.

$$\kappa=\frac{P_o-P_e}{1-P_e}.$$

Cohen's **Category weighting** defaults to `none`. `linear` and `quadratic` use agreement weights $1-|i-j|/(K-1)$ and $1-(i-j)^2/(K-1)^2$. Provide numerical categories in increasing order, or identical explicitly ordered ordinal level lists. Ordinal lists retain unobserved levels; numeric codes are treated as ordered, equally spaced category positions. Plain text has no inferred weighted order. Fleiss is unweighted.

The single `result` includes kappa, observed/expected agreement, category labels and rater counts; Cohen also returns a contingency matrix in that category order. Asymptotic standard errors and a two-sided normal confidence interval use the configured confidence level (default 0.95). Cohen uses multinomial delta variance; Fleiss uses subject-level delta variance. A separate normal test of $H_0:\kappa=0$ uses null variance. Normal intervals can exceed the coefficient's theoretical range. Degenerate chance agreement of 1 fails; unavailable zero-variance inference is null. Negative kappa is retained.

References: [statsmodels Cohen Kappa](https://www.statsmodels.org/stable/generated/statsmodels.stats.inter_rater.cohens_kappa.html), [R Fleiss Kappa](https://search.r-project.org/CRAN/refmans/irr/html/kappam.fleiss.html).
