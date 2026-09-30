# Intraclass correlation (ICC)

Each **Rater** input is a Numeric measurement column. Rows identify the same subjects across all columns. At least two subjects and two raters are required; at most 64 raters are supported. Use complete aligned data.

**ICC definition** defaults to `ICC2`:

| Choice       | Model and definition               | Measurement               |
| ------------ | ---------------------------------- | ------------------------- |
| ICC1 / ICC1k | One-way random, absolute agreement | Single / mean of k raters |
| ICC2 / ICC2k | Two-way random, absolute agreement | Single / mean of k raters |
| ICC3 / ICC3k | Two-way mixed, consistency         | Single / mean of k raters |

For n subjects and k raters, let $MS_B,MS_J,MS_E,MS_W$ be subject, rater, two-way residual, and one-way within-subject mean squares. The single-measure estimators are

$$ICC1=\frac{MS_B-MS_W}{MS_B+(k-1)MS_W},$$
$$ICC2=\frac{MS_B-MS_E}{MS_B+(k-1)MS_E+k(MS_J-MS_E)/n},$$
$$ICC3=\frac{MS_B-MS_E}{MS_B+(k-1)MS_E}.$$

Average-measure denominators are respectively $MS_B$, $MS_B+(MS_J-MS_E)/n$, and $MS_B$. The zero-ICC F test uses $MS_B/MS_W$ with df $(n-1,n(k-1))$ for ICC1, or $MS_B/MS_E$ with df $(n-1,(n-1)(k-1))$ otherwise. Confidence intervals use F distributions at the selected level (default 0.95) and a Satterthwaite approximation for ICC2.

`result` explicitly records model, measurement, agreement definition, coefficient, ANOVA mean squares, inference, and interval. Negative estimates are retained. Nonpositive coefficient denominators and no variation fail. Infinite-limit F statistics and unavailable intervals are null. These definitions do not expose two-way mixed absolute-agreement ICC.

Reference: [Shrout and Fleiss (1979)](https://doi.org/10.1037/0033-2909.86.2.420), [reference implementation](https://github.com/raphaelvallat/pingouin/blob/main/src/pingouin/reliability.py).
