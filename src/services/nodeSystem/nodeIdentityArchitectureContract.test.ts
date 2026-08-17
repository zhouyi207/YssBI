import { readFileSync, readdirSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import * as ts from 'typescript/unstable/ast';
import {
  SymbolFlags,
  type Checker,
  type NodeHandle,
  type Project,
  type Symbol as TypeScriptSymbol,
  type Type,
} from 'typescript/unstable/sync';
import { describe, expect, it } from 'vitest';
import {
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
} from '@/tests/helpers/typescriptAudit';

const sourceRoot = resolve('src');
const auditPath = 'src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts';
const functionPinSource = 'src/shared/types/domain/functionSignaturePin.ts';
const dataTypeSource = 'src/shared/types/domain/dataType.ts';
const functionSignatureDtoSource = 'src/shared/types/dto/editorMutation.ts';
const graphDataStoreSource = 'src/features/core/dataStore/graphDataStore.ts';
const nodeDetailPanelSource = 'src/views/EditorView/Layout/Detail/panels/NodeDetailPanel.tsx';

const fixtureSupportSources = {
  [functionPinSource]: readFileSync(resolve(functionPinSource), 'utf8'),
  [dataTypeSource]: readFileSync(resolve(dataTypeSource), 'utf8'),
  [functionSignatureDtoSource]: readFileSync(resolve(functionSignatureDtoSource), 'utf8'),
  [graphDataStoreSource]: readFileSync(resolve(graphDataStoreSource), 'utf8'),
} as const;

interface SemanticContext {
  checker: Checker;
  project: Project;
}

function resolveNode(handle: NodeHandle | undefined, project: Project): ts.Node | undefined {
  return handle?.resolve(project);
}

function symbolDeclarations(symbol: TypeScriptSymbol, project: Project): ts.Node[] {
  return symbol.declarations.flatMap((handle) => {
    const declaration = resolveNode(handle, project);
    return declaration ? [declaration] : [];
  });
}

function symbolValueDeclaration(
  symbol: TypeScriptSymbol,
  project: Project,
): ts.Node | undefined {
  return resolveNode(symbol.valueDeclaration, project);
}

function isFixtureDirectory(projectRelativeToSource: string): boolean {
  return projectRelativeToSource === 'tests/fixtures'
    || projectRelativeToSource.startsWith('tests/fixtures/');
}

function productionFiles(directory = sourceRoot): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      if (isFixtureDirectory(relative(sourceRoot, path).replace(/\\/g, '/'))) return [];
      return productionFiles(path);
    }
    const projectPath = relative(resolve('.'), path).replace(/\\/g, '/');
    if (!['.ts', '.tsx'].includes(extname(path))
      || /\.test\.[^.]+$/.test(path)
      || projectPath === auditPath) return [];
    return [projectPath];
  });
}

function unwrapExpression(node: ts.Expression): ts.Expression {
  if (ts.isParenthesizedExpression(node)
    || ts.isAssertionExpression(node)
    || ts.isSatisfiesExpression(node)) {
    return unwrapExpression(node.expression);
  }
  return node;
}

type StaticValue = string | readonly string[];

function createStaticEvaluator(sourceFile: ts.SourceFile, context: SemanticContext) {
  const { checker, project } = context;
  const evaluate = (
    node: ts.Expression,
    visiting: ReadonlySet<ts.VariableDeclaration> = new Set(),
    depth = 0,
  ): StaticValue | null => {
    if (depth > 32) return null;
    const expression = unwrapExpression(node);
    if (ts.isStringLiteralLikeNode(expression)) return expression.text;
    if (ts.isIdentifier(expression)) {
      const symbol = checker.getSymbolAtLocation(expression);
      if (!symbol || (symbol.flags & SymbolFlags.Alias) !== 0) return null;
      const declaration = symbolValueDeclaration(symbol, project);
      if (!declaration
        || !ts.isVariableDeclaration(declaration)
        || declaration.getSourceFile() !== sourceFile
        || !declaration.initializer
        || !ts.isVariableDeclarationList(declaration.parent)
        || (declaration.parent.flags & ts.NodeFlags.Const) === 0
        || visiting.has(declaration)) return null;
      return evaluate(
        declaration.initializer,
        new Set([...visiting, declaration]),
        depth + 1,
      );
    }
    if (ts.isArrayLiteralExpression(expression)) {
      const values = expression.elements.map((element) =>
        ts.isExpression(element) ? evaluate(element, visiting, depth + 1) : null);
      return values.every((value): value is string => typeof value === 'string')
        ? values
        : null;
    }
    if (ts.isBinaryExpression(expression) && expression.operatorToken.kind === ts.SyntaxKind.PlusToken) {
      const left = evaluate(expression.left, visiting, depth + 1);
      const right = evaluate(expression.right, visiting, depth + 1);
      return typeof left === 'string' && typeof right === 'string' ? left + right : null;
    }
    if (ts.isCallExpression(expression)
      && expression.arguments.length <= 1
      && ts.isPropertyAccessExpression(expression.expression)
      && expression.expression.name.text === 'join') {
      const array = evaluate(expression.expression.expression, visiting, depth + 1);
      const separator = expression.arguments.length === 0
        ? ','
        : evaluate(expression.arguments[0], visiting, depth + 1);
      return Array.isArray(array) && typeof separator === 'string'
        ? array.join(separator)
        : null;
    }
    if (ts.isTemplateExpression(expression)) {
      const values = expression.templateSpans.map((span) =>
        evaluate(span.expression, visiting, depth + 1));
      if (!values.every((value): value is string => typeof value === 'string')) return null;
      return expression.head.text + expression.templateSpans
        .map((span, index) => `${values[index]}${span.literal.text}`)
        .join('');
    }
    return null;
  };

  return (node: ts.Expression): string | null => {
    const value = evaluate(node);
    return typeof value === 'string' ? value : null;
  };
}

function propertyName(property: ts.ObjectLiteralElementLike): string | null {
  if (ts.isSpreadAssignment(property)) return null;
  const name = property.name;
  if (!name) return null;
  if (ts.isIdentifier(name) || ts.isStringLiteralLikeNode(name)) return name.text;
  return null;
}

function calledIdentifier(node: ts.CallExpression): string | null {
  const expression = unwrapExpression(node.expression);
  return ts.isIdentifier(expression) ? expression.text : null;
}


function symbolAtExpression(
  expression: ts.Expression,
  checker: Checker,
): TypeScriptSymbol | undefined {
  const callable = unwrapExpression(expression);
  const location = ts.isPropertyAccessExpression(callable) ? callable.name : callable;
  return checker.getSymbolAtLocation(location);
}

function moduleExport(
  moduleSpecifier: ts.Expression,
  exportName: string,
  checker: Checker,
): TypeScriptSymbol | undefined {
  const moduleSymbol = checker.getSymbolAtLocation(moduleSpecifier);
  return moduleSymbol
    ? checker.getExportsOfModule(moduleSymbol).find((symbol) => symbol.name === exportName)
    : undefined;
}

function symbolTargetsCanonical(
  symbol: TypeScriptSymbol | undefined,
  context: SemanticContext,
  sourcePath: string,
  exportName: string,
  visiting: ReadonlySet<number> = new Set(),
): boolean {
  if (!symbol || visiting.has(symbol.id)) return false;
  const { checker, project } = context;
  const nextVisiting = new Set([...visiting, symbol.id]);
  const declarations = symbolDeclarations(symbol, project);
  if (symbol.name === exportName
    && declarations.some((declaration) =>
      declaration.getSourceFile().fileName.replace(/\\/g, '/').endsWith(sourcePath))) {
    return true;
  }
  if ((symbol.flags & SymbolFlags.Alias) !== 0) {
    const target = checker.getAliasedSymbol(symbol);
    if (target.id !== symbol.id
      && !checker.isUnknownSymbol(target)
      && symbolTargetsCanonical(target, context, sourcePath, exportName, nextVisiting)) {
      return true;
    }
  }
  return declarations.some((declaration) => {
    if (ts.isVariableDeclaration(declaration) && declaration.initializer) {
      return symbolTargetsCanonical(
        symbolAtExpression(declaration.initializer, checker),
        context,
        sourcePath,
        exportName,
        nextVisiting,
      );
    }
    if (ts.isImportSpecifier(declaration)) {
      const importDeclaration = declaration.parent.parent.parent;
      if (!ts.isImportDeclaration(importDeclaration)) return false;
      const importedName = (declaration.propertyName ?? declaration.name).text;
      return symbolTargetsCanonical(
        moduleExport(importDeclaration.moduleSpecifier, importedName, checker),
        context,
        sourcePath,
        exportName,
        nextVisiting,
      );
    }
    if (ts.isExportSpecifier(declaration)) {
      const exportDeclaration = declaration.parent.parent;
      if (!ts.isExportDeclaration(exportDeclaration) || !exportDeclaration.moduleSpecifier) {
        return false;
      }
      const importedName = (declaration.propertyName ?? declaration.name).text;
      return symbolTargetsCanonical(
        moduleExport(exportDeclaration.moduleSpecifier, importedName, checker),
        context,
        sourcePath,
        exportName,
        nextVisiting,
      );
    }
    return false;
  });
}

function expressionTargetsGraphDataStore(
  expression: ts.Expression,
  context: SemanticContext,
): boolean {
  return symbolTargetsCanonical(
    symbolAtExpression(expression, context.checker),
    context,
    graphDataStoreSource,
    'useGraphDataStore',
  );
}

function callTargets(
  node: ts.CallExpression,
  context: SemanticContext,
  sourcePath: string,
  exportName: string,
): boolean {
  const { checker, project } = context;
  const symbol = symbolAtExpression(node.expression, checker);
  if (symbolTargetsCanonical(symbol, context, sourcePath, exportName)) return true;
  const signatureDeclaration = resolveNode(
    checker.getResolvedSignature(node)?.declaration,
    project,
  );
  if (signatureDeclaration
    && signatureDeclaration.getSourceFile().fileName.replace(/\\/g, '/').endsWith(sourcePath)
    && ts.isFunctionDeclaration(signatureDeclaration)
    && signatureDeclaration.name?.text === exportName) return true;
  return (!symbol || checker.isUnknownSymbol(symbol) || symbol.declarations.length === 0)
    && calledIdentifier(node) === exportName;
}

function callReturnsFunctionSignaturePin(
  node: ts.CallExpression,
  checker: Checker,
): boolean {
  const type = checker.getTypeAtLocation(node);
  if (!type) return false;
  return [type.getAliasSymbol(), type.getSymbol()]
    .filter((symbol): symbol is TypeScriptSymbol => symbol != null)
    .some((symbol) => symbol.name === 'FunctionSignaturePin');
}

function objectLiteralKind(node: ts.Expression): string | null {
  const expression = unwrapExpression(node);
  if (!ts.isObjectLiteralExpression(expression)) return null;
  const kind = expression.properties.find((property) => propertyName(property) === 'kind');
  return kind && ts.isPropertyAssignment(kind) && ts.isStringLiteralLikeNode(kind.initializer)
    ? kind.initializer.text
    : null;
}

function typeContainsFunctionSignatureDto(
  type: Type | undefined,
  context: SemanticContext,
  visiting: ReadonlySet<number> = new Set(),
): boolean {
  if (!type || visiting.has(type.id)) return false;
  const { checker, project } = context;
  const nextVisiting = new Set([...visiting, type.id]);
  const symbols = [type.getAliasSymbol(), type.getSymbol()].filter(
    (symbol): symbol is TypeScriptSymbol => symbol != null,
  );
  if (symbols.some((symbol) =>
    (symbol.name === 'FunctionSignatureDto' || symbol.name === 'FunctionParameterDto')
    && symbolDeclarations(symbol, project).some((candidate) =>
      candidate.getSourceFile().fileName.replace(/\\/g, '/').endsWith(functionSignatureDtoSource)))) {
    return true;
  }
  if (type.isUnionType() || type.isIntersectionType()) {
    return type.getTypes().some((member) =>
      typeContainsFunctionSignatureDto(member, context, nextVisiting));
  }
  if (type.isTypeReference()
    && symbols.some((symbol) => symbol.name === 'Array' || symbol.name === 'ReadonlyArray')) {
    return checker.getTypeArguments(type).some((argument) =>
      typeContainsFunctionSignatureDto(argument, context, nextVisiting));
  }
  return false;
}

function createRawSignatureTaint(context: SemanticContext) {
  const { checker, project } = context;
  const isRawExpression = (
    node: ts.Expression,
    visiting: ReadonlySet<number> = new Set(),
  ): boolean => {
    const expression = unwrapExpression(node);
    if (ts.isPropertyAccessExpression(expression)) {
      const owner = unwrapExpression(expression.expression);
      return isRawExpression(owner, visiting)
        || typeContainsFunctionSignatureDto(checker.getTypeAtLocation(owner), context)
        || typeContainsFunctionSignatureDto(checker.getTypeAtLocation(expression), context);
    }
    if (ts.isIdentifier(expression)) {
      const symbol = checker.getSymbolAtLocation(expression);
      if (!symbol || visiting.has(symbol.id)) {
        return typeContainsFunctionSignatureDto(checker.getTypeAtLocation(expression), context);
      }
      const nextVisiting = new Set([...visiting, symbol.id]);
      const declaration = symbolValueDeclaration(symbol, project);
      if (declaration && ts.isVariableDeclaration(declaration) && declaration.initializer
        && isRawExpression(declaration.initializer, nextVisiting)) return true;
      if (typeContainsFunctionSignatureDto(checker.getTypeAtLocation(expression), context)) {
        return true;
      }
      if (declaration && ts.isParameterDeclaration(declaration)) {
        const callback = declaration.parent;
        const call = callback.parent;
        if ((ts.isArrowFunction(callback) || ts.isFunctionExpression(callback))
          && ts.isCallExpression(call)
          && call.arguments.includes(callback)) {
          const callee = unwrapExpression(call.expression);
          return ts.isPropertyAccessExpression(callee)
            && callee.name.text === 'map'
            && isRawExpression(callee.expression, nextVisiting);
        }
      }
      return false;
    }
    if (ts.isCallExpression(expression)) {
      return callTargets(expression, context, dataTypeSource, 'dataTypeFromDisplayString')
        && expression.arguments.some((argument) => isRawExpression(argument, visiting));
    }
    if (ts.isBinaryExpression(expression)
      && expression.operatorToken.kind === ts.SyntaxKind.QuestionQuestionToken) {
      return isRawExpression(expression.left, visiting);
    }
    if (ts.isConditionalExpression(expression)) {
      return isRawExpression(expression.whenTrue, visiting)
        || isRawExpression(expression.whenFalse, visiting);
    }
    return false;
  };
  return isRawExpression;
}

function returnedExpression(callback: ts.Expression): ts.Expression | null {
  const expression = unwrapExpression(callback);
  if (!ts.isArrowFunction(expression) && !ts.isFunctionExpression(expression)) return null;
  if (!ts.isBlock(expression.body)) return unwrapExpression(expression.body);
  const returned = expression.body.statements.find(ts.isReturnStatement)?.expression;
  return returned ? unwrapExpression(returned) : null;
}

function returnedObjectLiteral(callback: ts.Expression): ts.ObjectLiteralExpression | null {
  const returned = returnedExpression(callback);
  return returned && ts.isObjectLiteralExpression(returned) ? returned : null;
}

function directSignaturePinMapping(
  node: ts.CallExpression,
  isRawExpression: (node: ts.Expression) => boolean,
): ts.ObjectLiteralExpression | null {
  const expression = unwrapExpression(node.expression);
  if (!ts.isPropertyAccessExpression(expression)
    || expression.name.text !== 'map'
    || !isRawExpression(expression.expression)) return null;
  const projected = node.arguments[0] && returnedObjectLiteral(node.arguments[0]);
  if (!projected) return null;
  const fields = new Set(
    projected.properties.map(propertyName).filter((name): name is string => name !== null),
  );
  return ['id', 'name', 'dataType'].every((field) => fields.has(field)) ? projected : null;
}

function rawSignaturePinCallMapping(
  node: ts.CallExpression,
  isRawExpression: (node: ts.Expression) => boolean,
): ts.CallExpression | null {
  const expression = unwrapExpression(node.expression);
  if (!ts.isPropertyAccessExpression(expression)
    || expression.name.text !== 'map'
    || !isRawExpression(expression.expression)) return null;
  const returned = node.arguments[0] && returnedExpression(node.arguments[0]);
  if (!returned || !ts.isCallExpression(returned) || returned.arguments.length < 3) return null;
  return returned.arguments.slice(0, 3).every((argument) => isRawExpression(argument))
    ? returned
    : null;
}


function expressionPropertyName(
  node: ts.Expression,
  staticString: (node: ts.Expression) => string | null,
): string | null {
  const expression = unwrapExpression(node);
  if (ts.isPropertyAccessExpression(expression)) return expression.name.text;
  if (ts.isElementAccessExpression(expression) && expression.argumentExpression) {
    return staticString(expression.argumentExpression);
  }
  return null;
}

function readsGraphEntities(
  node: ts.Expression,
  context: SemanticContext,
  staticString: (node: ts.Expression) => string | null,
  visiting: ReadonlySet<number> = new Set(),
): boolean {
  const { checker, project } = context;
  const expression = unwrapExpression(node);
  if (expressionPropertyName(expression, staticString) === 'graphEntities') return true;
  if (!ts.isIdentifier(expression)) return false;

  const symbol = checker.getSymbolAtLocation(expression);
  if (!symbol || visiting.has(symbol.id)) return expression.text === 'graphEntities';
  const nextVisiting = new Set([...visiting, symbol.id]);
  return symbolDeclarations(symbol, project).some((declaration) => {
    if (ts.isVariableDeclaration(declaration) && declaration.initializer) {
      return readsGraphEntities(declaration.initializer, context, staticString, nextVisiting);
    }
    if (ts.isBindingElement(declaration)) {
      const name = declaration.propertyName ?? declaration.name;
      return name !== undefined && ts.isIdentifier(name) && name.text === 'graphEntities';
    }
    return false;
  });
}

function nodeDetailScopedLookupFinding(
  node: ts.Node,
  context: SemanticContext,
  staticString: (node: ts.Expression) => string | null,
): string | null {

  if ((ts.isForInStatement(node) || ts.isForOfStatement(node))
    && readsGraphEntities(node.expression, context, staticString)) {
    return 'node detail lookup is not graphPath and nodeId scoped';
  }
  if (ts.isCallExpression(node)) {
    const callee = unwrapExpression(node.expression);
    const owner = ts.isPropertyAccessExpression(callee) || ts.isElementAccessExpression(callee)
      ? unwrapExpression(callee.expression)
      : null;
    const method = expressionPropertyName(node.expression, staticString);
    const isObjectEnumeration = owner && ts.isIdentifier(owner)
      && ((owner.text === 'Object'
        && ['entries', 'values', 'keys', 'getOwnPropertyNames', 'getOwnPropertySymbols'].includes(method ?? ''))
        || (owner.text === 'Reflect' && method === 'ownKeys'));
    if (isObjectEnumeration
      && node.arguments.some((argument) => readsGraphEntities(argument, context, staticString))) {
      return 'node detail lookup is not graphPath and nodeId scoped';
    }
  }

  return null;
}

function sourceOffendersFromSourceFile(
  path: string,
  sourceFile: ts.SourceFile,
  context: SemanticContext,
): string[] {
  const { checker } = context;
  const staticString = createStaticEvaluator(sourceFile, context);
  const isRawExpression = createRawSignatureTaint(context);
  const offenders: string[] = [];
  const report = (node: ts.Node, finding: string) => {
    const line = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
    offenders.push(`${path}:${line}: ${finding}`);
  };

  const auditsGraphMutationBoundary = path.includes('/features/application/editorMutation/');
  const auditsNodeDetailPanel = path.replace(/\\/g, '/').endsWith(nodeDetailPanelSource);
  const visit = (node: ts.Node): void => {
    if (auditsNodeDetailPanel) {
      const finding = nodeDetailScopedLookupFinding(node, context, staticString);
      if (finding) report(node, finding);
    }
    if (auditsGraphMutationBoundary && ts.isCallExpression(node)) {
      const expression = unwrapExpression(node.expression);
      if (ts.isPropertyAccessExpression(expression)
        && expression.name.text === 'setState'
        && expressionTargetsGraphDataStore(expression.expression, context)) {
        report(node, 'graph mutation bypasses the application boundary');
      }
    }

    if (ts.isCallExpression(node)) {
      const projectsRawSignature = node.arguments.some((argument) =>
        isRawExpression(argument));
      if (path !== functionPinSource
        && projectsRawSignature
        && (callTargets(node, context, functionPinSource, 'createDataSignaturePin')
          || callReturnsFunctionSignaturePin(node, checker))) {
        report(node, 'function signature-to-pin mapping');
        if (node.arguments.some((argument) => staticString(argument) === 'Result')) {
          report(node, 'fixed function output Result');
        }
      }
      if (callTargets(node, context, dataTypeSource, 'dataTypeFromDisplayString')
        && projectsRawSignature) {
        report(node, 'function signature display-type parsing');
      }
      const directMapping = directSignaturePinMapping(node, isRawExpression);
      if (directMapping) {
        report(directMapping, 'function signature-to-pin mapping');
        const name = directMapping.properties.find((property) => propertyName(property) === 'name');
        if (name && ts.isPropertyAssignment(name) && staticString(name.initializer) === 'Result') {
          report(name, 'fixed function output Result');
        }
      }
      const callMapping = rawSignaturePinCallMapping(node, isRawExpression);
      if (callMapping) report(callMapping, 'function signature-to-pin mapping');
    }
    if (ts.isBinaryExpression(node)
      && node.operatorToken.kind === ts.SyntaxKind.QuestionQuestionToken
      && objectLiteralKind(node.right) === 'Any'
      && isRawExpression(node.left)) {
      report(node, 'function interface Any fallback');
    }
    if (ts.isObjectLiteralExpression(node)) {
      const fields = new Set(node.properties.map(propertyName).filter((name): name is string => name !== null));
      if (['nodeTypeId', 'resourcePath', 'createArgs'].every((field) => fields.has(field))) {
        report(node, 'resource-bound descriptor synthesis');
      }
    }
    if (ts.isBinaryExpression(node)
      && node.operatorToken.kind === ts.SyntaxKind.PlusToken
      && staticString(node.left) === 'variables/') {
      report(node, 'variable resource path synthesis');
    }
    if (ts.isTemplateExpression(node) && node.head.text === 'variables/') {
      report(node, 'variable resource path synthesis');
    }
    node.forEachChild(visit);
  };
  visit(sourceFile);
  return [...new Set(offenders)];
}

function sourceOffendersFromSource(path: string, source: string): string[] {
  return withIsolatedTypeScriptProject({ [path]: source }, ({ project, sourceFile }) => (
    sourceOffendersFromSourceFile(path, sourceFile(path), {
      checker: project.checker,
      project,
    })
  ));
}

function sourceOffendersFromFixture(sources: Record<string, string>): string[] {
  return withIsolatedTypeScriptProject(
    { ...fixtureSupportSources, ...sources },
    ({ project, sourceFile }) => Object.keys(sources).flatMap((path) => (
      sourceOffendersFromSourceFile(path, sourceFile(path), {
        checker: project.checker,
        project,
      })
    )),
  );
}

function productionSourceOffenders(paths: string[]): string[] {
  return withProductionTypeScriptProject(({ project, sourceFile }) => (
    paths.flatMap((path) => sourceOffendersFromSourceFile(path, sourceFile(path), {
      checker: project.checker,
      project,
    }))
  ));
}

describe('frontend stable node identity architecture audit behavior', () => {
  it('rejects resource-bound descriptor construction', () => {
    const source = 'const descriptor = { resourcePath: path, createArgs: args, nodeTypeId: typeId };';
    expect(sourceOffendersFromSource('src/features/example.ts', source))
      .toContainEqual(expect.stringContaining('resource-bound descriptor synthesis'));
  });

  it('does not confuse DTO type declarations with descriptor construction', () => {
    const source = 'interface Descriptor { nodeTypeId: string; resourcePath: string; createArgs: unknown }';
    expect(sourceOffendersFromSource('src/shared/types/example.ts', source)).toEqual([]);
  });

  it.each<Record<string, string>>([
    {
      'src/__architecture_fixture__/entry.ts': `
        import { createDataSignaturePin } from '../shared/types/domain/functionSignaturePin';
        export const value = createDataSignaturePin('value', 'Value', { kind: 'Int64' });
      `,
    },
    {
      'src/__architecture_fixture__/bridge.ts': `
        export { createDataSignaturePin as makePin } from '../shared/types/domain/functionSignaturePin';
      `,
      'src/__architecture_fixture__/entry.ts': `
        import { makePin } from './bridge';
        export const value = makePin('value', 'Value', { kind: 'Int64' });
      `,
    },
    {
      'src/__architecture_fixture__/entry.ts': `
        import { createDataSignaturePin } from '../shared/types/domain/functionSignaturePin';
        export function makeValue(signature: string) {
          return createDataSignaturePin(signature, 'Value', { kind: 'Int64' });
        }
      `,
    },
    {
      'src/__architecture_fixture__/entry.ts': `
        import { dataTypeFromDisplayString } from '../shared/types/domain/dataType';
        interface ColumnMetadata { return_type: string }
        export const parseColumn = (column: ColumnMetadata) =>
          dataTypeFromDisplayString(column.return_type);
      `,
    },
    {
      'src/__architecture_fixture__/bridge.ts': `
        export { createDataSignaturePin as makePin } from '../shared/types/domain/functionSignaturePin';
      `,
      'src/__architecture_fixture__/entry.ts': `
        import { makePin as dynamicPin } from './bridge';
        export function makeValue(signature: string, label: string) {
          return dynamicPin(signature, label, { kind: 'Int64' });
        }
      `,
    },
    {
      'src/__architecture_fixture__/entry.ts': `
        import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
        import { dataTypeFromDisplayString } from '../shared/types/domain/dataType';
        interface Envelope<T> {
          payload: T;
          metadata: { return_type: string };
        }
        export const parseMetadata = (envelope: Envelope<FunctionSignatureDto>) =>
          dataTypeFromDisplayString(envelope.metadata.return_type);
      `,
    },
  ])('allows ordinary pin constructors and signature-independent re-exports %#', (sources) => {
    expect(sourceOffendersFromFixture(sources)).toEqual([]);
  });

  it.each([
    [
      {
        'src/__architecture_fixture__/entry.ts': `
          import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
          import { createDataSignaturePin as renamedPin } from '../shared/types/domain/functionSignaturePin';
          export const project = (signature: FunctionSignatureDto) =>
            signature.parameters.map((parameter) =>
              renamedPin(parameter.id, parameter.name, { kind: 'Int64' }));
        `,
      },
      'function signature-to-pin mapping',
    ],
    [
      {
        'src/__architecture_fixture__/entry.ts': `
          import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
          import * as pins from '../shared/types/domain/functionSignaturePin';
          export const project = (signature: FunctionSignatureDto) =>
            signature.parameters.map((parameter) =>
              pins.createDataSignaturePin(parameter.id, parameter.name, { kind: 'Int64' }));
        `,
      },
      'function signature-to-pin mapping',
    ],
    [
      {
        'src/__architecture_fixture__/entry.ts': `
          import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
          import { createDataSignaturePin } from '../shared/types/domain/functionSignaturePin';
          const renamedPin = createDataSignaturePin;
          export const project = (signature: FunctionSignatureDto) =>
            signature.parameters.map((parameter) =>
              renamedPin(parameter.id, parameter.name, { kind: 'Int64' }));
        `,
      },
      'function signature-to-pin mapping',
    ],
    [
      {
        'src/__architecture_fixture__/bridge.ts': `
          export { createDataSignaturePin as movedPin } from '../shared/types/domain/functionSignaturePin';
        `,
        'src/__architecture_fixture__/entry.ts': `
          import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
          import { movedPin } from './bridge';
          import { dataTypeFromDisplayString } from '../shared/types/domain/dataType';
          export const project = (signature: FunctionSignatureDto) =>
            signature.parameters.map((parameter) => movedPin(
              parameter.id,
              parameter.name,
              dataTypeFromDisplayString(parameter.type_name) ?? { kind: 'Any' },
            ));
        `,
      },
      'function signature-to-pin mapping',
    ],
    [
      {
        'src/__architecture_fixture__/helper.ts': `
          import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
          import { createDataSignaturePin as buildPin } from '../shared/types/domain/functionSignaturePin';
          import { dataTypeFromDisplayString as parseType } from '../shared/types/domain/dataType';
          export const movedHelper = (rawSignature: FunctionSignatureDto) =>
            rawSignature.parameters.map((parameter) => buildPin(
              parameter.id,
              parameter.name,
              parseType(parameter.type_name) ?? { kind: 'Any' },
            ));
        `,
        'src/__architecture_fixture__/entry.ts': `
          import { movedHelper as renamedHelper } from './helper';
          renamedHelper(currentSignature);
        `,
      },
      'function signature-to-pin mapping',
    ],
    [
      {
        'src/__architecture_fixture__/entry.ts': `
          import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
          import { dataTypeFromDisplayString as parseType } from '../shared/types/domain/dataType';
          export const project = (signature: FunctionSignatureDto) =>
            parseType(signature.return_type ?? 'Any');
        `,
      },
      'function signature display-type parsing',
    ],
    [
      {
        'src/__architecture_fixture__/entry.ts': `
          import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
          export const project = (signature: FunctionSignatureDto) =>
            signature.parameters.map((parameter) => ({
              id: parameter.id,
              name: parameter.name,
              dataType: { kind: 'Int64' as const },
            }));
        `,
      },
      'function signature-to-pin mapping',
    ],
  ])('uses symbol binding and projection structure to reject evasive mapping fixture %#', (sources, finding) => {
    expect(sourceOffendersFromFixture(sources))
      .toContainEqual(expect.stringContaining(finding));
  });

  it('tracks parser results through aliases into Any fallback and fixed Result projection', () => {
    const offenders = sourceOffendersFromFixture({
      'src/__architecture_fixture__/entry.ts': `
        import type { FunctionSignatureDto } from '../shared/types/dto/editorMutation';
        import { createDataSignaturePin as makePin } from '../shared/types/domain/functionSignaturePin';
        import { dataTypeFromDisplayString as parseType } from '../shared/types/domain/dataType';
        function project(signature: FunctionSignatureDto) {
          const parser = parseType;
          const parsed = parser(signature.return_type);
          const projectedType = parsed ?? { kind: 'Any' };
          return makePin('return', 'Result', projectedType);
        }
      `,
    });

    expect(offenders).toEqual(expect.arrayContaining([
      expect.stringContaining('function signature display-type parsing'),
      expect.stringContaining('function interface Any fallback'),
      expect.stringContaining('function signature-to-pin mapping'),
      expect.stringContaining('fixed function output Result'),
    ]));
  });


  it.each([
    {
      'src/features/application/editorMutation/example.ts': `
        import { useGraphDataStore as graphStore } from '../../core/dataStore/graphDataStore';
        graphStore.setState((state) => ({ graphEntities: state.graphEntities }));
      `,
    },
    {
      'src/features/application/editorMutation/example.ts': `
        import { useGraphDataStore } from '../../core/dataStore/graphDataStore';
        const graphStore = useGraphDataStore;
        graphStore.setState((state) => ({ graphEntities: state.graphEntities }));
      `,
    },
    {
      'src/features/application/editorMutation/example.ts': `
        import * as graphStores from '../../core/dataStore/graphDataStore';
        graphStores.useGraphDataStore.setState((state) => ({ graphEntities: state.graphEntities }));
      `,
    },
    {
      'src/features/application/editorMutation/graphStoreBridge.ts': `
        export { useGraphDataStore as graphAuthority } from '../../core/dataStore/graphDataStore';
      `,
      'src/features/application/editorMutation/example.ts': `
        import { graphAuthority } from './graphStoreBridge';
        graphAuthority.setState((state) => ({ graphEntities: state.graphEntities }));
      `,
    },
  ])('keeps graph mutations behind the application boundary %#', (sources) => {
    expect(sourceOffendersFromFixture(sources as Record<string, string>))
      .toContainEqual(expect.stringContaining('graph mutation bypasses the application boundary'));
  });

  it.each([
    `declare const state: { graphEntities: Record<string, { nodes: Record<string, unknown> }> };
     Object.entries(state.graphEntities).find(([, bucket]) => bucket.nodes[nodeId]);`,
    `declare const state: { graphEntities: Record<string, { nodes: Record<string, unknown> }> };
     const entities = state.graphEntities;
     Object.values(entities).find((bucket) => bucket.nodes[nodeId]);`,
    `declare const state: { graphEntities: Record<string, { nodes: Record<string, unknown> }> };
     for (const graphPath in state.graphEntities) void state.graphEntities[graphPath].nodes[nodeId];`,
  ])('requires graphPath and nodeId scoped NodeDetailPanel lookup %#', (source) => {
    expect(sourceOffendersFromSource(nodeDetailPanelSource, source)).toContainEqual(
      expect.stringContaining('node detail lookup is not graphPath and nodeId scoped'),
    );
  });

  it('allows graph-path selection and non-graph object enumeration in NodeDetailPanel', () => {
    const source = `
      declare const state: {
        graphEntities: Record<string, {
          nodes: Record<string, { capabilities: Record<string, boolean> }>
        }>
      };
      const node = state.graphEntities[graphPath]?.nodes[nodeId];
      const enabledCapabilities = Object.entries(node.capabilities)
        .filter(([, enabled]) => enabled);
      const localHelpers = { useFunctionCatalog: () => enabledCapabilities };
      localHelpers.useFunctionCatalog();
      const localActions = { updateCallFunctionTarget: () => undefined };
      const updateTarget = localActions.updateCallFunctionTarget;
      const { updateCallFunctionTarget } = localActions;
      updateTarget();
      updateCallFunctionTarget();
      const panel = <Panel onSelect={localActions.updateCallFunctionTarget} />;
      void panel;
    `;
    expect(sourceOffendersFromSource(nodeDetailPanelSource, source)).toEqual([]);
  });

  it('excludes only the exact tests/fixtures directory segment', () => {
    expect(isFixtureDirectory('tests/fixtures')).toBe(true);
    expect(isFixtureDirectory('tests/fixtures/catalog.json')).toBe(true);
    expect(isFixtureDirectory('tests/fixturesProduction')).toBe(false);
    expect(isFixtureDirectory('tests/fixturesProduction/offender.ts')).toBe(false);
  });
});

describe('frontend stable node identity architecture', () => {
  it('keeps production identity, descriptor, and mutation boundaries intact', () => {
    expect(productionSourceOffenders(productionFiles())).toEqual([]);
  }, 30_000);
});
