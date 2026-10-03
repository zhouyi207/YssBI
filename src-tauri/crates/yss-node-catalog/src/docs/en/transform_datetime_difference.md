# Date Difference

Connect Series and Other, a Datetime scalar or aligned series. Series inputs must have equal lengths and pair by current position, including mixed database and in-memory inputs. Unit defaults to day; week, hour, minute, second, millisecond, microsecond and nanosecond are available.

Result is Series minus Other, expressed as a Float64 duration in the chosen unit; fractions are retained. Null propagates. Calendar values use the project's timezone-free interpretation.
