# Sampling weights

Connect numeric values. `input_kind=weights` (default) accepts strictly positive finite sampling weights. `inclusion_probabilities` accepts finite probabilities in (0,1] and computes 1/probability. Missing values, zero weights and nonfinite results fail; no records are dropped.

**result** reports observation count, original weight sum, minimum/maximum, Kish effective sample size `(Σw)²/Σw²`, unequal-weighting design effect `n/Kish effective sample size`, and population-denominator weight coefficient of variation. There is no hypothesis test or p value; Kish quantities exclude the complete effects of stratification and clustering.

**weights** returns validated weights or inverse inclusion probabilities, retaining every row and alignment with source data for connection to survey analysis nodes. Output weights are not normalized. There is no fixed row ceiling.

Does not construct stratified/clustered designs, nonresponse adjustments, poststratification or raking weights. Inclusion probabilities are sampling probabilities, not treatment propensity scores. Sampling weights differ from WLS precision weights.
