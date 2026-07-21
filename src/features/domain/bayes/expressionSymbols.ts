import type { BayesSymbolRoleDTO, ExpressionDTO, RawExpressionDTO, SymbolDraftDTO } from '@/shared/types/bayes';

export interface ExpressionSymbols {
  dataVariables: string[];
  parameters: string[];
}

const COMMON_DEPENDENT_SYMBOLS = new Set(['y']);
const COMMON_INDEPENDENT_SYMBOLS = new Set(['x', 't', 'time']);

export function collectRawSymbols(expression: RawExpressionDTO | null): string[] {
  const symbols = new Set<string>();
  visitRawExpression(expression, symbols);
  return Array.from(symbols).sort();
}

export function createSymbolDrafts(
  symbolNames: readonly string[],
  existing: readonly SymbolDraftDTO[] = [],
  datasetColumnNames: readonly string[] = [],
): SymbolDraftDTO[] {
  const existingByName = new Map(existing.map(symbol => [symbol.name, symbol]));
  const columnNames = new Set(datasetColumnNames);
  return Array.from(new Set(symbolNames))
    .sort()
    .map(name => {
      const previous = existingByName.get(name);
      if (previous) return previous;
      const inferredRole = inferSymbolRole(name, columnNames);
      return { name, role: inferredRole, inferredRole, userEdited: false };
    });
}

export function collectExpressionSymbols(expression: ExpressionDTO | null): ExpressionSymbols {
  const dataVariables = new Set<string>();
  const parameters = new Set<string>();
  visitBoundExpression(expression, dataVariables, parameters);
  return {
    dataVariables: Array.from(dataVariables).sort(),
    parameters: Array.from(parameters).sort(),
  };
}

export function bindRawExpression(
  expression: RawExpressionDTO | null,
  symbols: readonly SymbolDraftDTO[],
): ExpressionDTO | null {
  if (!expression) return null;
  const roles = new Map(symbols.map(symbol => [symbol.name, symbol.role]));
  return bindExpressionNode(expression, roles);
}

export function bindResponseExpression(expression: RawExpressionDTO): ExpressionDTO {
  const responseNames = collectRawSymbols(expression);
  if (responseNames.length !== 1) {
    throw new Error(`Response expression must contain exactly one symbol, received ${responseNames.length}`);
  }
  return bindExpressionNode(expression, new Map([[responseNames[0], 'dependent']]));
}

export function responseBaseNameFromRaw(expression: RawExpressionDTO): string {
  const symbols = collectRawSymbols(expression);
  if (symbols.length !== 1) {
    throw new Error(`Response expression must contain exactly one symbol, received ${symbols.length}`);
  }
  return symbols[0];
}

export function symbolNamesByRole(symbols: readonly SymbolDraftDTO[], role: BayesSymbolRoleDTO): string[] {
  return symbols.filter(symbol => symbol.role === role).map(symbol => symbol.name).sort();
}

function inferSymbolRole(name: string, datasetColumnNames: Set<string>): BayesSymbolRoleDTO {
  if (COMMON_DEPENDENT_SYMBOLS.has(name)) return 'dependent';
  if (datasetColumnNames.has(name) || COMMON_INDEPENDENT_SYMBOLS.has(name)) return 'independent';
  return 'parameter';
}

function bindExpressionNode(expression: RawExpressionDTO, roles: Map<string, BayesSymbolRoleDTO>): ExpressionDTO {
  switch (expression.type) {
    case 'number':
      return expression;
    case 'symbol': {
      const role = roles.get(expression.name) ?? 'parameter';
      return role === 'independent' || role === 'dependent'
        ? { type: 'data_variable', name: expression.name }
        : { type: 'parameter', name: expression.name };
    }
    case 'unary':
      return { ...expression, arg: bindExpressionNode(expression.arg, roles) };
    case 'binary':
      return {
        ...expression,
        left: bindExpressionNode(expression.left, roles),
        right: bindExpressionNode(expression.right, roles),
      };
    case 'call':
      return { ...expression, args: expression.args.map(arg => bindExpressionNode(arg, roles)) };
  }
}

function visitRawExpression(expression: RawExpressionDTO | null, symbols: Set<string>): void {
  if (!expression) return;
  switch (expression.type) {
    case 'symbol':
      symbols.add(expression.name);
      return;
    case 'unary':
      visitRawExpression(expression.arg, symbols);
      return;
    case 'binary':
      visitRawExpression(expression.left, symbols);
      visitRawExpression(expression.right, symbols);
      return;
    case 'call':
      expression.args.forEach(arg => visitRawExpression(arg, symbols));
      return;
    case 'number':
      return;
  }
}

function visitBoundExpression(expression: ExpressionDTO | null, dataVariables: Set<string>, parameters: Set<string>): void {
  if (!expression) return;
  switch (expression.type) {
    case 'column':
    case 'data_variable':
      dataVariables.add(expression.name);
      return;
    case 'parameter':
      parameters.add(expression.name);
      return;
    case 'unary':
      visitBoundExpression(expression.arg, dataVariables, parameters);
      return;
    case 'binary':
      visitBoundExpression(expression.left, dataVariables, parameters);
      visitBoundExpression(expression.right, dataVariables, parameters);
      return;
    case 'call':
      expression.args.forEach(arg => visitBoundExpression(arg, dataVariables, parameters));
      return;
    case 'number':
      return;
  }
}
