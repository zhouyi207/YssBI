# Rank Values

Connect a Numeric, Text, Datetime or Ordinal series. Optional Context allows partition and order columns from the same row domain. With no order columns, rank the series itself; otherwise use the requested columns. Ascending order and Null last are the defaults; Descending and Nulls first apply in both cases. Ordinal values follow their declared level order.

Ordinary ranking assigns equal ranks to ties and leaves gaps; Dense ranking removes the gaps. Result is Numeric and retains source row order and alignment.
