# Inverse Standardize DataSeries

Map a standardized **Float64** **DataSeries** back to its original scale using a **Transform** handle from **Standardize DataSeries**:

$$
x = z \cdot \sigma + \mu
$$

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataSeries** | Input | Standardized **DataSeries**\<Float64\> |
| **Transform** | Input | `StandardizeTransform1D` from **Standardize DataSeries** |
| **Result** | Output | Series restored to original units |

## Usage

Wire **Transform** from the same **Standardize DataSeries** node that produced the standardized values. Use after prediction or processing on z-scored data when downstream nodes or reports need interpretable magnitudes.
