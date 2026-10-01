# Survival calibration curve

Connect aligned `time` and `event` series. Time must be finite and strictly positive, in one consistent unit; `event=1` (or true) means an observed event and `0` (or false) means right censoring. Rows with missing values are rejected, not dropped. Censoring is assumed independent of the event process, conditional on the model/group used. Equal-length columns from unrelated tables are not treated as aligned.

Connect numeric `predicted_risk` in [0,1], representing the probability of an event by `survival_horizon=1` (strictly positive). The risk and observed follow-up must refer to the same outcome and horizon. `calibration_bins=10`, an integer at least 2, requests approximately equal-size bins sorted by prediction. Equal predictions are never split; the realized count may be smaller, including one.

## Calculation and output

For each bin $b$, plot its mean predicted risk against $\widehat p_b=1-\widehat S_b(t)$, where $\widehat S_b$ is Kaplan–Meier at the horizon. The 45-degree reference line represents perfect calibration. The line between points is visual interpolation, not a fitted calibration model.

`result` is plot data with axis labels, point metadata, horizon and `bins`. Each bin contains its sample size, prediction range/mean, observed risk, and a 95% interval obtained by reversing the Kaplan–Meier log-log interval. No significance test or calibration slope is computed. Censor-only rows remain in risk sets.

A bin with residual survival but no follow-up support reaching the horizon is rejected; bins with survival already zero can support later horizons. Do not choose a horizon beyond supported follow-up. To evaluate a model output, select `time`, `event`, `risk` from its `predictions` DataFrame. Fitting-sample calibration is apparent performance; external or held-out predictions are needed for out-of-sample assessment. The node does not refit or cross-validate the model.
