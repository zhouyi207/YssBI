# Word Cloud

Words takes a tokenized Text series, one complete term per row. Case is preserved and surrounding whitespace is trimmed. Empty terms and missing values are rejected.

The default shows the 100 most frequent terms, configurable from 1 to 256. Counts use all input rows; ties are ordered by term. Larger frequencies produce larger text. The node does not tokenize sentences or Chinese passages.

Open the result output in a workbench result panel or a separate Plot window.
