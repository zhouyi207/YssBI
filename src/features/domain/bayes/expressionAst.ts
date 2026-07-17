import { BINARY_OPERATOR_LABELS, type ExpressionDTO, type RawExpressionDTO } from '@/shared/types/bayes';

export function formatExpression(expression: ExpressionDTO | null): string {
  if (!expression) return '';
  switch (expression.type) {
    case 'number':
      return Number.isInteger(expression.value) ? String(expression.value) : String(expression.value);
    case 'column':
    case 'data_variable':
    case 'parameter':
      return expression.name;
    case 'unary':
      return `-${formatExpressionWithParens(expression.arg)}`;
    case 'binary':
      return `${formatExpressionWithParens(expression.left)} ${BINARY_OPERATOR_LABELS[expression.op]} ${formatExpressionWithParens(expression.right)}`;
    case 'call':
      return `${expression.function}(${expression.args.map(formatExpression).join(', ')})`;
  }
}

export function formatRawExpression(expression: RawExpressionDTO | null): string {
  if (!expression) return '';
  switch (expression.type) {
    case 'number':
      return Number.isInteger(expression.value) ? String(expression.value) : String(expression.value);
    case 'symbol':
      return expression.name;
    case 'unary':
      return `-${formatRawExpressionWithParens(expression.arg)}`;
    case 'binary':
      return `${formatRawExpressionWithParens(expression.left)} ${BINARY_OPERATOR_LABELS[expression.op]} ${formatRawExpressionWithParens(expression.right)}`;
    case 'call':
      return `${expression.function}(${expression.args.map(formatRawExpression).join(', ')})`;
  }
}

function formatExpressionWithParens(expression: ExpressionDTO): string {
  if (expression.type === 'binary') {
    return `(${formatExpression(expression)})`;
  }
  return formatExpression(expression);
}

function formatRawExpressionWithParens(expression: RawExpressionDTO): string {
  if (expression.type === 'binary') {
    return `(${formatRawExpression(expression)})`;
  }
  return formatRawExpression(expression);
}
