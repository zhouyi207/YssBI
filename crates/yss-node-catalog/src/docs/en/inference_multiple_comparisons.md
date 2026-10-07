# Post-hoc multiple comparisons

Connect a finite numeric response and aligned nonmissing groups. Group labels may be numeric, categorical, ordinal, binary, text or identifiers. At least two groups are required; rows represent independent observations, not repeated measurements.

equal_variances defaults to true. All pairs use the common one-way ANOVA residual variance $MSE=\sum_g\sum_i(y_{gi}-\bar y_g)^2/(N-G)$, requiring $N>G$. For groups a and b, $SE=\sqrt{MSE(1/n_a+1/n_b)}$ and $t=(\bar y_a-\bar y_b)/SE$ uses $N-G$ degrees of freedom.

When equal_variances is false, each group needs at least two observations. Welch uses $SE^2=v_a+v_b$, $v_g=s_g^2/n_g$, and $df=(v_a+v_b)^2/[v_a^2/(n_a-1)+v_b^2/(n_b-1)]$. Every comparison tests $H_0:\mu_a=\mu_b$ against a two-sided difference using the t distribution. Zero comparison SE cannot support this inference.

adjustment defaults to holm: sort m raw P values and apply $p^*_{(i)}=\min(1,\max_{j\le i}(m-j+1)p_{(j)})$. bonferroni uses $\min(1,mp)$; none leaves P values unadjusted. Holm and Bonferroni control family-wise error for this complete pair family when the individual tests are valid. confidence_level defaults to 0.95, strictly between 0 and 1; compare adjusted_p_value to $1-c$.

Each interval is the mean difference plus/minus its t critical value times SE. Bonferroni uses tail probability $(1-c)/(2m)$ and simultaneous family coverage; Holm and none retain nominal, unadjusted intervals. Read intervals_adjusted before interpreting coverage.

result contains group_labels in first-appearance order, per-group counts/means/SDs, method, adjustment, confidence_level, comparisons and intervals_adjusted. The comparisons table contains group_a/group_b (one-based positions in group_labels), estimate (a minus b), standard_error, degrees_of_freedom, statistic, p_value, adjusted_p_value, lower and upper. The node compares raw group means; it does not perform Tukey's range test or covariate-adjusted contrasts.
