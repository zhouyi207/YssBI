# Panel Fit

Connect response, ordered predictors, entity IDs and time values as aligned finite numeric series. Entity-time pairs must be unique; observations are sorted by entity and time for estimation.

Select estimator, effects, intercept and covariance:

- fixed_effects, lsdv and random_effects support entity, time and two_way effects.
- first_difference supports entity effects and uses consecutive integer time values.
- between supports entity or time effects and only nonrobust covariance.
- maximum_likelihood supports all three effect dimensions and only nonrobust covariance.
- lsdv requires an intercept. Other supported estimators allow nonrobust, HC0–HC3 or cluster covariance.

Invalid combinations and failed estimation are reported explicitly. Output model contains coefficients, covariance and estimator-specific inference/group statistics. Connect it to Panel Summary. Observation-level fitted/residual series are not exposed.
