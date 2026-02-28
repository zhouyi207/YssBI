/** Supported column types for cast dropdown (value = backend dtype string) */
export const COLUMN_TYPE_OPTIONS = [
  { label: 'Int8', value: 'Int8' },
  { label: 'Int16', value: 'Int16' },
  { label: 'Int32', value: 'Int32' },
  { label: 'Int64', value: 'Int64' },
  { label: 'UInt8', value: 'UInt8' },
  { label: 'UInt16', value: 'UInt16' },
  { label: 'UInt32', value: 'UInt32' },
  { label: 'UInt64', value: 'UInt64' },
  { label: 'Float32', value: 'Float32' },
  { label: 'Float64', value: 'Float64' },
  { label: 'Boolean', value: 'Boolean' },
  { label: 'String', value: 'String' },
  { label: 'DateTime', value: 'DateTime' },
] as const;
