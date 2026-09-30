# Date Difference

Connect Series and Other, a Datetime scalar or aligned series. Unit defaults to day; week, hour, minute, second, millisecond, microsecond and nanosecond are available.

Result is Series minus Other, expressed as a Float64 duration in the chosen unit; fractions are retained. Null propagates. Calendar values use the project's timezone-free interpretation.
