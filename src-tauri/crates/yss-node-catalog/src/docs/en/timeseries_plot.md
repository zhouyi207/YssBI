# Time-series plot

Connect one finite numeric `series` in time order. The optional `time` input accepts one aligned finite numeric series, strictly increasing without duplicate coordinates. If omitted, the horizontal coordinates are 1, 2, ..., n. Convert calendar values to numeric coordinates upstream when an explicit time axis is needed. Missing values are rejected and input rows are not silently reordered.

The output `result` is line-plot data: `data` contains x/y points, `xLabel` and `yLabel` identify the axes, `referenceLines` is empty, and `metadata` records original and displayed counts. No parameters, fitting, stationarity test, confidence interval or p-value are produced.

All input rows are accepted subject to the execution memory budget. The shared chart display samples at most 2,048 points evenly across the ordered sequence, retaining endpoints and recording sampling in metadata; this is a display limit rather than a limit on observations used by statistical nodes. Use the original series for model fitting. Inspect trends, seasonality, abrupt level shifts and unusual observations before selecting a model.
