# To Datetime

Convert a scalar or series to Datetime calendar semantics. Scalars remain scalar; series retain row order and alignment.

## Parameters

- **Calendar Kind** defaults to Auto, preserving existing Date, Time or Datetime inputs. Text defaults to Datetime. Date, Time and Datetime can also be selected explicitly.
- **Calendar Precision** defaults to microseconds for newly parsed values; seconds, milliseconds and nanoseconds are available.
- **Calendar Format** defaults to empty for ISO parsing. Custom formats use strftime, for example `%d/%m/%Y` or `%d/%m/%Y %H:%M:%S %z`, with a maximum of 256 characters.

## Conversion rules

Timezone offsets are removed while retaining calendar and clock fields, without converting to UTC. Precision loss fails; conversion to Date cannot discard a nonzero time component. Numeric Unix epochs are not inferred. Nulls remain null.

## Output and execution

The output meaning is fixed to Datetime. Resolved calendar representation and precision travel with the output. Materialized inputs are converted immediately; lazy series validate actual values when consumed. This node produces derived results without editing the source dataset.
