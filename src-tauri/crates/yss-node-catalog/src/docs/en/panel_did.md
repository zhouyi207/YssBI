# Panel DID (TWFE)

Two-way fixed effects difference-in-differences for a $2\times2$ design.

Regresses **Y** on optional **X** and **Treat×Post** only — main effects of Treat and Post are absorbed by entity and time FE:

$$
Y_{it} = \alpha_i + \gamma_t + \beta (Treat_i \times Post_t) + X_{it}'\delta + \varepsilon_{it}
$$

This Fit node consumes `response`, `predictors`, `entity`, `time`, and `treatment`, with `event_study` and `placebo_repetitions` parameters. Outputs `model`, `fitted`, `residuals`, and `report`. Its execution kernel is not yet registered.
