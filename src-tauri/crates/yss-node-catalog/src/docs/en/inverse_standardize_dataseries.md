# Inverse Standardize DataSeries

Map a standardized **Float64** **DataSeries** back to its original scale using a **Transform** handle from **Standardize DataSeries**:

$$
x = z \cdot \sigma + \mu
$$

## Usage

Wire **Transform** from the same **Standardize DataSeries** node that produced the standardized values. Use after prediction or processing on z-scored data when downstream nodes or reports need interpretable magnitudes.
