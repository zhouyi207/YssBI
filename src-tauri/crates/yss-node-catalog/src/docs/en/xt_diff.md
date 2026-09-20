# Panel Difference

Connect the context DataFrame and a numeric series from the same row domain. Select Entity Column and Time Column from the frame. Rows are ordered within each entity by the time key, differenced repeatedly according to Order, and restored to original row positions. Missing/duplicate entity-time keys fail. Leading and missing-dependent values remain Null; rows are never dropped and calendar gaps are not filled. No separate alignment node is required.
