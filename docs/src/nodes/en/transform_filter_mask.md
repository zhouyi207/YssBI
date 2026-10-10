# Filter by Series

Source and the Binary Mask must have the same row count and pair by current position; the mask may come from another database series or an in-memory calculation. With Remove matching rows off, keep only true rows; false and Null rows are removed. With it on, remove true rows and retain false and Null rows.

Combine comparisons with AND, OR and NOT to express multiple conditions. Result has a new row domain and leaves the source unchanged.
