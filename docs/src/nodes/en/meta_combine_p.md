# Combine p-values

Connect p*values in $[0,1]$; studies must be independent and test a scientifically compatible null. p_method defaults to fisher. Its statistic $X=-2\sum\log p_i$ has a $\chi^2*{2k}$ distribution under the joint null that all component null hypotheses hold; the alternative is departure in at least one study.
stouffer combines consistently oriented one-sided P values: $Z=\sum w_i\Phi^{-1}(1-p_i)/\sqrt{\sum w_i^2}\sim N(0,1)$ under the null, against a positive directional alternative. An optional weights input supplies positive finite weights; disconnected weights mean equal weighting. Fisher ignores weights.
result contains method, studies, statistic, degrees_of_freedom and p_value. A limiting infinite statistic is null with its limiting P value retained; simultaneous 0 and 1 in Stouffer is undefined. This output is a combined significance level, not an effect size.
