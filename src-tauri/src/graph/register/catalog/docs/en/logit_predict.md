# Logit Predict

Applies a fitted **LogitModel** to new data.

## Inputs

- **Model** from **Logit**
- Dynamic exog pins (same names/types as estimation)

## Output

**Probability** — $P(y=1) = \Lambda(x'\hat\beta)$ as `DataSeries<Float64>`.

Connect **Model** first to reveal input pins.
