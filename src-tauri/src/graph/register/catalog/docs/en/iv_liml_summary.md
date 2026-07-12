# IV:LIML Summary

Limited Information Maximum Likelihood IV estimator (Stata `ivregress liml`). Same input layout as **IV:2SLS Summary**.

## Inputs

| Pin | Description |
|-----|-------------|
| **In** | Execution flow entry |
| **Y** | Dependent variable |
| **X:exogs** | Exogenous regressors (repeatable `DataSeries`) |
| **X:endog** | Endogenous regressors (`DataFrame`, one column per endogenous variable) |
| **x_instruments** | Instruments (`DataFrame`) |
| **Config** | Optional **IV:2SLS Configure** |
| **Time** | Optional time index (required for some VCE choices) |

## Output

| Pin | Description |
|-----|-------------|
| **Result** | **OLSResult** |
| **Out** | Execution flow exit |

Opens the LIML summary window after estimation. LIML is often used when instruments are weak (finite-sample bias of 2SLS).
