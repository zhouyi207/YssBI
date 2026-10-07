# Box Plot

Add 1–64 Numeric series. Samples are independent and may have different lengths. Empty, missing or nonfinite samples are rejected. Labels use source column names when available and series numbers otherwise.

Quartiles use linear interpolation. The box spans Q1–Q3 with a median line. Whiskers reach the most extreme observations inside $[Q_1-1.5\,IQR,Q_3+1.5\,IQR]$, where $IQR=Q_3-Q_1$. At most 2048 outliers are drawn; the outlier count uses the complete sample.

Open the result output in a workbench result panel or a separate Plot window.
