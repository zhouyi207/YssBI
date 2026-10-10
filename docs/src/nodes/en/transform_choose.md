# Conditional Selection

Connect a Binary Condition and When True / When False values. Branches may be scalars or aligned series of the same semantic type, with compatible physical representations. Scalars broadcast across the condition's row domain.

A true condition selects When True; false or Null selects When False. Series inputs must have equal lengths and pair by current position, including mixed database and in-memory inputs. Null branch values are retained.
