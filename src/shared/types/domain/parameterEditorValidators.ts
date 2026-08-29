import type {
  FilterLiteralDto,
  FilterOperatorDto,
  FilterPredicateDto,
  RelationalScalarTypeDto,
  SchemaAwareParameterEditorDto,
} from '@/shared/types/domain/editorProjection';

const relationalScalarTypes = new Set<RelationalScalarTypeDto>([
  'boolean', 'int64', 'float64', 'string', 'date', 'dateTime', 'unknown',
]);
const filterOperators = new Set<FilterOperatorDto>([
  'equal', 'notEqual', 'lessThan', 'lessThanOrEqual', 'greaterThan',
  'greaterThanOrEqual', 'isNull', 'isNotNull',
]);

function hasExactKeys(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => actual.includes(key));
}

function isColumnOption(value: unknown, withOperators: boolean): boolean {
  const keys = withOperators
    ? ['name', 'dataType', 'operators', 'literalTypes']
    : ['name', 'dataType'];
  if (!hasExactKeys(value, keys)) return false;
  return typeof value.name === 'string'
    && relationalScalarTypes.has(value.dataType as RelationalScalarTypeDto)
    && (!withOperators || (Array.isArray(value.operators)
      && value.operators.every((operator) => filterOperators.has(operator as FilterOperatorDto))
      && Array.isArray(value.literalTypes)
      && value.literalTypes.every((type) => (
        type === 'boolean' || type === 'integer' || type === 'decimal' || type === 'string'
      ))));
}

function isFilterLiteral(value: unknown): value is FilterLiteralDto {
  if (!hasExactKeys(value, ['type', 'value'])) return false;
  if (value.type === 'boolean') return typeof value.value === 'boolean';
  return (value.type === 'integer' || value.type === 'decimal' || value.type === 'string')
    && typeof value.value === 'string';
}

function isFilterPredicate(value: unknown): value is FilterPredicateDto {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  const nullCheck = candidate.operator === 'isNull' || candidate.operator === 'isNotNull';
  if (!hasExactKeys(candidate, nullCheck ? ['column', 'operator'] : ['column', 'operator', 'value'])) {
    return false;
  }
  return typeof candidate.column === 'string'
    && filterOperators.has(candidate.operator as FilterOperatorDto)
    && (nullCheck || isFilterLiteral(candidate.value));
}

export function isSchemaAwareParameterEditorDto(
  value: unknown,
): value is SchemaAwareParameterEditorDto {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  const commonValid = typeof candidate.available === 'boolean'
    && (candidate.unavailableReason === null || typeof candidate.unavailableReason === 'string');
  if (candidate.kind === 'projectColumns') {
    return hasExactKeys(candidate, [
      'kind', 'available', 'unavailableReason', 'options', 'value',
    ]) && commonValid
      && Array.isArray(candidate.options) && candidate.options.every((option) => isColumnOption(option, false))
      && Array.isArray(candidate.value) && candidate.value.every((column) => typeof column === 'string');
  }
  if (candidate.kind === 'filterPredicate') {
    return hasExactKeys(candidate, [
      'kind', 'available', 'unavailableReason', 'columns', 'value',
    ]) && commonValid
      && Array.isArray(candidate.columns) && candidate.columns.every((column) => isColumnOption(column, true))
      && (candidate.value === null || isFilterPredicate(candidate.value));
  }
  return false;
}
