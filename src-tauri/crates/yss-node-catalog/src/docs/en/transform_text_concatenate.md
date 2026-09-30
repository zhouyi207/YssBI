# Concatenate Text

Add at least two Text inputs in the requested order. Inputs may be scalars or series; at least one must be a series, and all series must share a row domain. Separator defaults to empty text.

Scalars broadcast. A Null input makes that row's result Null; empty strings participate normally. Output is Text in the shared row domain.
