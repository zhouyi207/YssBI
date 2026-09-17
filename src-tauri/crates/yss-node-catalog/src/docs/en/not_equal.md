# Not Equal (!=)

Tests whether two values differ:

$$
\text{Result} = (A \neq B)
$$

Uses the same element-wise comparison and broadcasting rules as Equal. Two scalars return a Binary scalar; any series input returns a Binary series. Non-null results are inverted; null remains null. Numeric representations compare exactly and text is never implicitly parsed as a number. Materialized series require equal lengths; lazy series require the same relation row domain.

## Usage

Produce a Boolean mask for filters or downstream data transformations when values differ.
