# Conditional Selection

Connect a Binary Condition and When True / When False values. Branches may be scalars or aligned series of the same semantic type, with compatible physical representations. Scalars broadcast across the condition's row domain.

A true condition selects When True; false or Null selects When False. The output is a series in the same row domain, with Null branch values retained.
