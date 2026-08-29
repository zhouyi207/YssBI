# Standardize DataSeries

Apply z-score standardization to a **Float64** **DataSeries**. Each value is scaled using the sample mean and standard deviation:

$$
z = \frac{x - \mu}{\sigma}
$$

The node also emits a **Transform** handle so the mapping can be reversed later.

## Usage

Connect a **Float64** **DataSeries**, run the graph, and wire **Standardized** to models or further transforms. Keep **Transform** when you need original-scale values—pass it to **Inverse Standardize DataSeries** together with standardized data.
