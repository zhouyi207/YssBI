import type { DataType } from '@/shared/types/domain/dataType';
import { DEFAULT_ARRAY_VALUE, DEFAULT_OBJECT_VALUE } from '@/shared/types/domain/dataType';
import type { DataValue } from '@/shared/types/domain/dataValue';
import { dataValueFromRaw, dataValueToRaw } from '@/shared/types/domain/dataValue';

export const DEFAULT_ARRAY_JSON = JSON.stringify(DEFAULT_ARRAY_VALUE, null, 2);

export const DEFAULT_OBJECT_JSON = JSON.stringify(DEFAULT_OBJECT_VALUE, null, 2);

export const DEFAULT_DATAFRAME_JSON = `{
  "col_0": [1, 2],
  "col_1": [3, 4]
}`;

export const DEFAULT_DATASERIES_JSON = `{
  "col_0": [1, 2, 3, 4]
}`;

export function isJsonLiteralContent(value: string): boolean {
  const t = value.trim();
  return t.startsWith('[') || t.startsWith('{');
}

export function getVariableLiteralPayload(dataType: DataType, dataValue: DataValue): string {
  if (dataType.kind === 'DataFrame' && dataValue.kind === 'DataFrame') {
    return dataValue.value;
  }
  if (dataType.kind === 'DataSeries' && dataValue.kind === 'DataSeries') {
    return typeof dataValue.value === 'string' ? dataValue.value : dataValue.value.id;
  }
  return '';
}

export function isVariableValueEmpty(dataType: DataType, dataValue: DataValue): boolean {
  if (dataValue.kind === 'Null') return true;
  switch (dataType.kind) {
    case 'Array':
      return dataValue.kind === 'Array' && dataValue.value.length === 0;
    case 'Object':
      return dataValue.kind === 'Object' && Object.keys(dataValue.value).length === 0;
    case 'DataFrame':
      if (dataValue.kind !== 'DataFrame') return true;
      return !dataValue.value.trim();
    case 'DataSeries': {
      if (dataValue.kind !== 'DataSeries') return true;
      const payload = getVariableLiteralPayload(dataType, dataValue);
      return !payload.trim();
    }
    default:
      return false;
  }
}

function tryParseJson(value: string): unknown | null {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function isColumnMapObject(parsed: unknown): parsed is Record<string, unknown> {
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return false;
  const obj = parsed as Record<string, unknown>;
  const keys = Object.keys(obj);
  if (keys.length === 0) return true;
  return keys.every((key) => key.trim().length > 0 && Array.isArray(obj[key]));
}

function getColumnMapRowCount(parsed: Record<string, unknown>): number | null {
  const keys = Object.keys(parsed);
  if (keys.length === 0) return 0;
  const lengths = keys.map((key) => (Array.isArray(parsed[key]) ? parsed[key].length : null));
  if (lengths.some((length) => length === null)) return null;
  const unique = new Set(lengths);
  if (unique.size !== 1) return null;
  return lengths[0] ?? 0;
}

function summarizeDataFrameJson(json: string): string | null {
  const parsed = tryParseJson(json);
  if (!isColumnMapObject(parsed)) return null;
  const colCount = Object.keys(parsed).length;
  const rowCount = getColumnMapRowCount(parsed);
  if (rowCount !== null && colCount > 0) {
    return `DataFrame(${rowCount} rows × ${colCount} cols)`;
  }
  if (rowCount === 0) return 'DataFrame(0 rows)';
  return 'DataFrame';
}

function summarizeDataSeriesJson(json: string): string | null {
  const parsed = tryParseJson(json);
  if (!isColumnMapObject(parsed)) return null;
  const keys = Object.keys(parsed);
  const colName = keys.length === 1 ? keys[0] : null;
  const rowCount = getColumnMapRowCount(parsed);
  if (colName && rowCount !== null) return `DataSeries(${colName}, ${rowCount})`;
  if (rowCount !== null) return `DataSeries(${rowCount})`;
  return colName ? `DataSeries(${colName})` : 'DataSeries';
}

export function formatVariableValueSummary(
  dataType: DataType,
  dataValue: DataValue,
  emptyLabel = '(empty)',
): string {
  if (isVariableValueEmpty(dataType, dataValue)) return emptyLabel;

  switch (dataType.kind) {
    case 'Array':
      if (dataValue.kind === 'Array') {
        return `Array(${dataValue.value.length})`;
      }
      return emptyLabel;
    case 'Object':
      if (dataValue.kind === 'Object') {
        const count = Object.keys(dataValue.value).length;
        return count > 0 ? `Object{${count}}` : 'Object{}';
      }
      return emptyLabel;
    case 'DataFrame': {
      const payload = getVariableLiteralPayload(dataType, dataValue);
      if (!payload) return emptyLabel;
      if (isJsonLiteralContent(payload)) {
        return summarizeDataFrameJson(payload) ?? 'DataFrame';
      }
      return payload;
    }
    case 'DataSeries': {
      const payload = getVariableLiteralPayload(dataType, dataValue);
      if (!payload) return emptyLabel;
      if (isJsonLiteralContent(payload)) {
        return summarizeDataSeriesJson(payload) ?? 'DataSeries';
      }
      return payload;
    }
    default:
      return String(dataValueToRaw(dataValue) ?? emptyLabel);
  }
}

function prettyJsonString(json: string, fallback: string): string {
  const parsed = tryParseJson(json);
  if (parsed === null) return fallback;
  return JSON.stringify(parsed, null, 2);
}

export function dataValueToEditableJson(dataType: DataType, dataValue: DataValue): string {
  if (dataType.kind === 'Array') {
    if (dataValue.kind === 'Array' && dataValue.value.length > 0) {
      const raw = dataValue.value.map((item) => dataValueToRaw(item));
      return JSON.stringify(raw, null, 2);
    }
    return DEFAULT_ARRAY_JSON;
  }
  if (dataType.kind === 'Object') {
    if (dataValue.kind === 'Object' && Object.keys(dataValue.value).length > 0) {
      return JSON.stringify(dataValue.value, null, 2);
    }
    return DEFAULT_OBJECT_JSON;
  }
  if (dataType.kind === 'DataFrame') {
    const payload = getVariableLiteralPayload(dataType, dataValue);
    if (payload && isJsonLiteralContent(payload)) {
      return prettyJsonString(payload, DEFAULT_DATAFRAME_JSON);
    }
    return DEFAULT_DATAFRAME_JSON;
  }
  if (dataType.kind === 'DataSeries') {
    const payload = getVariableLiteralPayload(dataType, dataValue);
    if (payload && isJsonLiteralContent(payload)) {
      return prettyJsonString(payload, DEFAULT_DATASERIES_JSON);
    }
    return DEFAULT_DATASERIES_JSON;
  }
  return '';
}

export function parseArrayValueFromJson(
  json: string,
  innerType: DataType,
): { ok: true; value: DataValue } | { ok: false; error: string } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return { ok: false, error: 'invalidJson' };
  }
  if (!Array.isArray(parsed)) {
    return { ok: false, error: 'notArray' };
  }
  return {
    ok: true,
    value: dataValueFromRaw(parsed, { kind: 'Array', inner: innerType }),
  };
}

export function parseObjectValueFromJson(
  json: string,
): { ok: true; value: DataValue } | { ok: false; error: string } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return { ok: false, error: 'invalidJson' };
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return { ok: false, error: 'notObject' };
  }
  return {
    ok: true,
    value: dataValueFromRaw(parsed, { kind: 'Object' }),
  };
}

function isDataFrameJsonShape(parsed: unknown): boolean {
  if (!isColumnMapObject(parsed)) return false;
  if (Object.keys(parsed).length === 0) return true;
  return getColumnMapRowCount(parsed) !== null;
}

function isDataSeriesJsonShape(parsed: unknown): boolean {
  if (!isColumnMapObject(parsed)) return false;
  const keys = Object.keys(parsed);
  if (keys.length === 0) return true;
  if (keys.length !== 1) return false;
  return getColumnMapRowCount(parsed) !== null;
}

function isEmptyColumnMapJson(compact: string): boolean {
  return compact === '{}';
}

export function parseDataFrameValueFromJson(
  json: string,
): { ok: true; value: DataValue } | { ok: false; error: string } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return { ok: false, error: 'invalidJson' };
  }
  if (!isDataFrameJsonShape(parsed)) {
    return { ok: false, error: 'notDataFrameContent' };
  }
  const compact = JSON.stringify(parsed);
  if (isEmptyColumnMapJson(compact)) {
    return { ok: true, value: { kind: 'Null' } };
  }
  return { ok: true, value: { kind: 'DataFrame', value: compact } };
}

export function parseDataSeriesValueFromJson(
  json: string,
): { ok: true; value: DataValue } | { ok: false; error: string } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return { ok: false, error: 'invalidJson' };
  }
  if (!isDataSeriesJsonShape(parsed)) {
    return { ok: false, error: 'notDataSeriesContent' };
  }
  const compact = JSON.stringify(parsed);
  if (isEmptyColumnMapJson(compact)) {
    return { ok: true, value: { kind: 'Null' } };
  }
  return { ok: true, value: { kind: 'DataSeries', value: compact } };
}

export function isJsonEditableVariableType(dataType: DataType): boolean {
  return ['Array', 'Object', 'DataFrame', 'DataSeries'].includes(dataType.kind);
}
