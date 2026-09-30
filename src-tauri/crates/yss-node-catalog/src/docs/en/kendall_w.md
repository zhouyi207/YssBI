# Kendall concordance W

Each **Rater** input is an aligned Numeric or Ordinal column, with each row identifying the same subject. At least two subjects and two raters are required, with at most 64 raters. Ordinal columns use their own explicit order; missing ratings fail.

Each rater's observations are ranked across subjects, averaging ties. With m raters, n subjects, summed subject ranks $R_i$, and $T=\sum_j\sum_g(t_{jg}^3-t_{jg})$,

$$W=\frac{12\sum_i[R_i-m(n+1)/2]^2}{m^2(n^3-n)-mT}.$$

W is between 0 and 1. Ties are always corrected. Under no concordance, $\chi^2=m(n-1)W$ has an approximate chi-square distribution with n-1 degrees of freedom. This approximation is less reliable for very small samples. If all raters give constant rankings, W is undefined and fails.

`result` includes W, sample/rater counts, tie correction, and the chi-square inference. This is multi-rater concordance, rather than pairwise Kendall tau-b.

Reference: [R Kendall concordance W](https://search.r-project.org/CRAN/refmans/irr/html/kendall.html).
