# VCE: HC1 (robust)

HC1 applies a degrees-of-freedom correction to HC0; also known as the default robust option in some packages.

## Formula

$$
\widehat{\mathrm{Var}}(\hat\beta) = \frac{n}{n-k} \cdot \widehat{\mathrm{Var}}_{\mathrm{HC0}}(\hat\beta)
$$

## Usage

Connect **VCE** → **OLS & WLS Configure** → **VCE**, then to **OLS** / **WLS** / Summary nodes.
