# Kernel Density Estimate

Values requires at least two finite Numeric observations and rejects missing values. Uses a Gaussian kernel with Silverman bandwidth $h=1.06\,s\,n^{-1/5}$, where $s$ is sample standard deviation. Zero variance uses unit bandwidth, enlarged by machine precision when the numerical scale cannot resolve a unit.

Density grid points defaults to 256 and ranges from 16 to 512. Grid size controls display resolution; density uses the complete sample. Result includes X, density and observation count and opens in a workbench result panel or separate Plot window.
