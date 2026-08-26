# TS Lag

Strict time-aligned lag (Stata `L.` semantics). Lags **Value Series** by **Lag** steps after aligning on **Time Series**.

When the time column is marked **Aligned** and matches the value length, re-alignment is skipped; otherwise **Interval** is inferred and values are aligned.

## Usage

First **Lag** observations are null. Duplicate time keys error; prefer **TS Align** for a regular grid first.
