# Probit Predict

Applies a fitted **ProbitModel** to new data.

## Inputs

- **Model** from **Probit**
- Dynamic exog pins matching estimation

## Output

**Probability** — $P(y=1) = \Phi(x'\hat\beta)$ as `DataSeries<Float64>`.

Connect **Model** first to reveal input pins.
