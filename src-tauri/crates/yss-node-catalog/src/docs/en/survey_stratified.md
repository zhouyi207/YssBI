# Stratified survey mean/proportion

Connect response and sampling weights, optionally adding one strata column and one clusters (primary sampling unit, PSU) column. This node requires strata.

Omitted strata means one stratum; omitted clusters treats every row as its own PSU. Identical PSU labels in different strata identify separate units. Columns must be aligned and complete, numbers finite, and sampling weights strictly positive. No rows are dropped.

Variance uses single-stage, with-replacement Taylor linearization, without finite-population corrections, multistage designs, replicate weights or subpopulation analysis. If stratum h has m_h PSUs and t_hj is a PSU score sum, variance sums `m_h/(m_h−1) × Σ(t_hj−stratum mean score)²` over strata. Regression uses outer-product matrices with inverse information on both sides. Design degrees of freedom equal PSU count minus stratum count.

`lonely_psu=fail` rejects single-PSU strata by default. Use `certainty` only for known certainty strata with no omitted later sampling stages; their variance contribution is zero. Overall design degrees of freedom must remain positive.

Weights are internally rescaled to mean 1. Multiplication by a common constant does not change estimates or standard errors. Design summaries retain original weight sum/range and Kish effective sample size `(Σw)²/Σw²`. `weight_design_effect=n/Kish effective sample size` describes unequal weighting alone, not the complete impact of stratification/clustering. There is no fixed row ceiling.

`statistic=mean` defaults to the weighted mean `Σwy/Σw`; `proportion` requires a 0/1 response and uses the same ratio estimate. Linearized scores are `w(y−estimate)/Σw`.

**result** returns estimate, design standard_error, two-sided 95% Wald t interval and design summary. The t distribution uses design degrees of freedom. This estimation node has no additional null hypothesis or p value. Proportion intervals are not clipped to [0,1] and may extend beyond it. When all responses are zero or all are one, SE is zero and `boundary_proportion` is true; this degenerate interval does not imply zero population uncertainty. Boundary proportions require a specialized interval method.

[Reference: survey summaries](https://r-survey.r-forge.r-project.org/pkgdown/docs/reference/surveysummary.html)
