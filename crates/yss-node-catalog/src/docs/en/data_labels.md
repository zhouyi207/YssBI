# Data labels (value meanings)

Connect a series to **input**, choose categorical or ordinal meaning, and enter exact codes and their labels.
**output** retains original codes and nulls. Labels travel as semantic metadata instead of replacing cells with label text.
For example, text code `001` can mean “Treatment”; its leading zeros remain intact.

Ordinal levels follow the configured order from low to high.
An empty configuration inherits a compatible existing domain. Categorical input without a domain is scanned in full during execution, giving each distinct non-null code a same-name label. Ordinal levels must still be supplied when none can be inherited.
Duplicate codes, invalid domains and nonnull observations outside a declared domain fail explicitly instead of becoming missing values.

To recode positive/negative codes as booleans, use To Binary.

Its configuration is stored in graph parameters and metadata follows the output; it does not directly edit source database field settings.

Series retain lazy relations and row alignment, with no fixed row cap.
These are value meanings. Rename columns with the existing column-rename operation and edit graph-node display names in node properties.
