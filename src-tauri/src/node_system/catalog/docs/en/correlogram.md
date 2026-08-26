# Correlogram (ACF & PACF)

Plots sample ACF and PACF of a **Float64** **DataSeries** up to **Lags** (default 20; capped at $n/2$).

Includes cumulative Ljung–Box $Q$ statistics and p-values (hover bars). 95% band half-width $1.96/\sqrt{n}$.

## Usage

Running the graph opens the **Plot** window with ACF / PACF panels. Requires at least 4 non-null observations.
