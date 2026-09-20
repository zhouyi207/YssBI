# Add Dummy Information

Attaches a reference-category hint to a Categorical, Ordinal or Binary series without changing its values or element type. Base Level is a stored category code, not a label; a supplied code must occur in the series. Empty means the downstream encoder chooses the reference category. The hint survives materialized series and Arrow table assembly. This node does not generate indicator columns; the current numeric regression nodes do not perform automatic dummy expansion.
