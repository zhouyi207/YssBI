# Map Values

Enter Source values and Replacement values as exact code text in matching order. Source values must be unique; replacements must fit the source physical type. Keep unmatched values defaults to true; turning it off makes unmatched values Null.

Result retains the row domain and element semantic. Declared category codes, positive-value and dummy reference metadata are updated by the mapping; equivalent target codes are merged. Binary results must still declare two distinct codes. Numeric bounds are cleared because replacements can change the range.
