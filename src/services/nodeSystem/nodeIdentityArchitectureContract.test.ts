import { readdirSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import * as ts from 'typescript';
import { describe, expect, it } from 'vitest';

const sourceRoot = resolve('src');
const auditPath = 'src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts';
const functionPinSource = 'src/shared/types/domain/functionSignaturePin.ts';
const dataTypeSource = 'src/shared/types/domain/dataType.ts';
const functionSignatureDtoSource = 'src/shared/types/dto/editorMutation.ts';
const graphDataStoreSource = 'src/features/core/dataStore/graphDataStore.ts';
const nodeDetailPanelSource = 'src/views/EditorView/Layout/Detail/panels/NodeDetailPanel.tsx';

const configPath = resolve('tsconfig.json');
const configFile = ts.readConfigFile(configPath, ts.sys.readFile);
if (configFile.error) throw new Error(ts.flattenDiagnosticMessageText(configFile.error.messageText, '\n'));
const parsedConfig = ts.parseJsonConfigFileContent(configFile.config, ts.sys, resolve('.'));

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
    || ts.isAsExpression(node)
    || ts.isTypeAssertionExpression(node)
    || ts.isSatisfiesExpression(node)) {
    return unwrapExpression(node.expression);
  }
  return node;
}

type StaticValue = string | readonly string[];

function createInMemoryProgram(path: string, source: string): {
  checker: ts.TypeChecker;
  sourceFile: ts.SourceFile;
} {
  const options: ts.CompilerOptions = {
    jsx: ts.JsxEmit.ReactJSX,
    module: ts.ModuleKind.ESNext,
    noLib: true,
    noResolve: true,
    target: ts.ScriptTarget.Latest,
  };
  const host: ts.CompilerHost = {
    fileExists: (fileName) => fileName === path,
    getCanonicalFileName: (fileName) => fileName,
    getCurrentDirectory: () => '',
    getDefaultLibFileName: () => 'lib.d.ts',
    getNewLine: () => '\n',
    getSourceFile: (fileName, languageVersion) => fileName === path
      ? ts.createSourceFile(
          fileName,
          source,
          languageVersion,
          true,
          path.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
        )
      : undefined,
    readFile: (fileName) => fileName === path ? source : undefined,
    useCaseSensitiveFileNames: () => true,
    writeFile: () => undefined,
  };
  const program = ts.createProgram([path], options, host);
  const sourceFile = program.getSourceFile(path);
  if (!sourceFile) throw new Error(`audit source '${path}' was not created`);
  return { checker: program.getTypeChecker(), sourceFile };
}

function createFixtureProgram(sources: Record<string, string>): ts.Program {
  const virtualSources = new Map(Object.entries(sources).map(([path, source]) => [
    resolve(path),
    source,
  ]));
  const host = ts.createCompilerHost(parsedConfig.options, true);
  const baseFileExists = host.fileExists.bind(host);
  const baseReadFile = host.readFile.bind(host);
  const baseGetSourceFile = host.getSourceFile.bind(host);
  host.fileExists = (fileName) => virtualSources.has(resolve(fileName)) || baseFileExists(fileName);
  host.readFile = (fileName) => virtualSources.get(resolve(fileName)) ?? baseReadFile(fileName);
  host.getSourceFile = (fileName, languageVersion, onError, shouldCreateNewSourceFile) => {
    const source = virtualSources.get(resolve(fileName));
    return source === undefined
      ? baseGetSourceFile(fileName, languageVersion, onError, shouldCreateNewSourceFile)
      : ts.createSourceFile(fileName, source, languageVersion, true, ts.ScriptKind.TS);
  };
  return ts.createProgram([...virtualSources.keys()], parsedConfig.options, host);
}

function createStaticEvaluator(sourceFile: ts.SourceFile, checker: ts.TypeChecker) {
  const evaluate = (
    node: ts.Expression,
    visiting: ReadonlySet<ts.VariableDeclaration> = new Set(),
    depth = 0,
  ): StaticValue | null => {
    if (depth > 32) return null;
    const expression = unwrapExpression(node);
    if (ts.isStringLiteralLike(expression)) return expression.text;
    if (ts.isIdentifier(expression)) {
      const symbol = checker.getSymbolAtLocation(expression);
      if (!symbol || (symbol.flags & ts.SymbolFlags.Alias) !== 0) return null;
      const declaration = symbol.valueDeclaration;
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
  const name = property.name;
  if (!name) return null;
  if (ts.isIdentifier(name) || ts.isStringLiteralLike(name)) return name.text;
  return null;
}

function calledIdentifier(node: ts.CallExpression): string | null {
  const expression = unwrapExpression(node.expression);
  return ts.isIdentifier(expression) ? expression.text : null;
}


function symbolAtExpression(
  expression: ts.Expression,
  checker: ts.TypeChecker,
): ts.Symbol | undefined {
  const callable = unwrapExpression(expression);
  const location = ts.isPropertyAccessExpression(callable) ? callable.name : callable;
  return checker.getSymbolAtLocation(location);
}

function moduleExport(
  moduleSpecifier: ts.Expression,
  exportName: string,
  checker: ts.TypeChecker,
): ts.Symbol | undefined {
  const moduleSymbol = checker.getSymbolAtLocation(moduleSpecifier);
  return moduleSymbol
    ? checker.getExportsOfModule(moduleSymbol).find((symbol) => symbol.getName() === exportName)
    : undefined;
}

function symbolTargetsCanonical(
  symbol: ts.Symbol | undefined,
  checker: ts.TypeChecker,
  sourcePath: string,
  exportName: string,
  visiting: ReadonlySet<ts.Symbol> = new Set(),
): boolean {
  if (!symbol || visiting.has(symbol)) return false;
  const nextVisiting = new Set([...visiting, symbol]);
  if (symbol.getName() === exportName
    && (symbol.declarations ?? []).some((declaration) =>
      declaration.getSourceFile().fileName.replace(/\\/g, '/').endsWith(sourcePath))) {
    return true;
  }
  if ((symbol.flags & ts.SymbolFlags.Alias) !== 0) {
    const target = checker.getAliasedSymbol(symbol);
    if (target !== symbol
      && symbolTargetsCanonical(target, checker, sourcePath, exportName, nextVisiting)) {
      return true;
    }
  }
  return (symbol.declarations ?? []).some((declaration) => {
    if (ts.isVariableDeclaration(declaration) && declaration.initializer) {
      return symbolTargetsCanonical(
        symbolAtExpression(declaration.initializer, checker),
        checker,
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
        checker,
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
        checker,
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
  checker: ts.TypeChecker,
): boolean {
  return symbolTargetsCanonical(
    symbolAtExpression(expression, checker),
    checker,
    graphDataStoreSource,
    'useGraphDataStore',
  );
}

function callTargets(
  node: ts.CallExpression,
  checker: ts.TypeChecker,
  sourcePath: string,
  exportName: string,
): boolean {
  const symbol = symbolAtExpression(node.expression, checker);
  if (symbolTargetsCanonical(symbol, checker, sourcePath, exportName)) return true;
  const signatureDeclaration = checker.getResolvedSignature(node)?.declaration;
  if (signatureDeclaration
    && signatureDeclaration.getSourceFile().fileName.replace(/\\/g, '/').endsWith(sourcePath)
    && 'name' in signatureDeclaration
    && signatureDeclaration.name
    && ts.isIdentifier(signatureDeclaration.name)
    && signatureDeclaration.name.text === exportName) return true;
  return !symbol?.declarations?.length && calledIdentifier(node) === exportName;
}

function callReturnsFunctionSignaturePin(
  node: ts.CallExpression,
  checker: ts.TypeChecker,
): boolean {
  const type = checker.getTypeAtLocation(node);
  return [type.aliasSymbol, type.getSymbol()]
    .filter((symbol): symbol is ts.Symbol => symbol != null)
    .some((symbol) => symbol.getName() === 'FunctionSignaturePin');
}

function objectLiteralKind(node: ts.Expression): string | null {
  const expression = unwrapExpression(node);
  if (!ts.isObjectLiteralExpression(expression)) return null;
  const kind = expression.properties.find((property) => propertyName(property) === 'kind');
  return kind && ts.isPropertyAssignment(kind) && ts.isStringLiteralLike(kind.initializer)
    ? kind.initializer.text
    : null;
}

function typeContainsFunctionSignatureDto(
  type: ts.Type,
  checker: ts.TypeChecker,
  visiting: ReadonlySet<ts.Type> = new Set(),
): boolean {
  if (visiting.has(type)) return false;
  const nextVisiting = new Set([...visiting, type]);
  const symbols = [type.aliasSymbol, type.getSymbol()].filter(
    (symbol): symbol is ts.Symbol => symbol != null,
  );
  if (symbols.some((symbol) =>
    (symbol.getName() === 'FunctionSignatureDto' || symbol.getName() === 'FunctionParameterDto')
    && (symbol.declarations ?? []).some((candidate) =>
      candidate.getSourceFile().fileName.replace(/\\/g, '/').endsWith(functionSignatureDtoSource)))) {
    return true;
  }
  if (type.isUnionOrIntersection()) {
    return type.types.some((member) =>
      typeContainsFunctionSignatureDto(member, checker, nextVisiting));
  }
  if ((type.flags & ts.TypeFlags.Object) !== 0
    && ((type as ts.ObjectType).objectFlags & ts.ObjectFlags.Reference) !== 0
    && symbols.some((symbol) => symbol.getName() === 'Array' || symbol.getName() === 'ReadonlyArray')) {
    return checker.getTypeArguments(type as ts.TypeReference).some((argument) =>
      typeContainsFunctionSignatureDto(argument, checker, nextVisiting));
  }
  return false;
}

function createRawSignatureTaint(checker: ts.TypeChecker) {
  const isRawExpression = (
    node: ts.Expression,
    visiting: ReadonlySet<ts.Symbol> = new Set(),
  ): boolean => {
    const expression = unwrapExpression(node);
    if (ts.isPropertyAccessExpression(expression)) {
      const owner = unwrapExpression(expression.expression);
      return isRawExpression(owner, visiting)
        || typeContainsFunctionSignatureDto(checker.getTypeAtLocation(owner), checker)
        || typeContainsFunctionSignatureDto(checker.getTypeAtLocation(expression), checker);
    }
    if (ts.isIdentifier(expression)) {
      const symbol = checker.getSymbolAtLocation(expression);
      if (!symbol || visiting.has(symbol)) {
        return typeContainsFunctionSignatureDto(checker.getTypeAtLocation(expression), checker);
      }
      const nextVisiting = new Set([...visiting, symbol]);
      const declaration = symbol.valueDeclaration;
      if (declaration && ts.isVariableDeclaration(declaration) && declaration.initializer
        && isRawExpression(declaration.initializer, nextVisiting)) return true;
      if (typeContainsFunctionSignatureDto(checker.getTypeAtLocation(expression), checker)) {
        return true;
      }
      if (declaration && ts.isParameter(declaration)) {
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
      return callTargets(expression, checker, dataTypeSource, 'dataTypeFromDisplayString')
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
  checker: ts.TypeChecker,
  staticString: (node: ts.Expression) => string | null,
  visiting: ReadonlySet<ts.Symbol> = new Set(),
): boolean {
  const expression = unwrapExpression(node);
  if (expressionPropertyName(expression, staticString) === 'graphEntities') return true;
  if (!ts.isIdentifier(expression)) return false;

  const symbol = checker.getSymbolAtLocation(expression);
  if (!symbol || visiting.has(symbol)) return expression.text === 'graphEntities';
  const nextVisiting = new Set([...visiting, symbol]);
  return (symbol.declarations ?? []).some((declaration) => {
    if (ts.isVariableDeclaration(declaration) && declaration.initializer) {
      return readsGraphEntities(declaration.initializer, checker, staticString, nextVisiting);
    }
    if (ts.isBindingElement(declaration)) {
      const name = declaration.propertyName ?? declaration.name;
      return ts.isIdentifier(name) && name.text === 'graphEntities';
    }
    return false;
  });
}

function nodeDetailScopedLookupFinding(
  node: ts.Node,
  checker: ts.TypeChecker,
  staticString: (node: ts.Expression) => string | null,
): string | null {

  if ((ts.isForInStatement(node) || ts.isForOfStatement(node))
    && readsGraphEntities(node.expression, checker, staticString)) {
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
      && node.arguments.some((argument) => readsGraphEntities(argument, checker, staticString))) {
      return 'node detail lookup is not graphPath and nodeId scoped';
    }
  }

  return null;
}

function sourceOffendersFromSourceFile(
  path: string,
  sourceFile: ts.SourceFile,
  checker: ts.TypeChecker,
): string[] {
  const staticString = createStaticEvaluator(sourceFile, checker);
  const isRawExpression = createRawSignatureTaint(checker);
  const offenders: string[] = [];
  const report = (node: ts.Node, finding: string) => {
    const line = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
    offenders.push(`${path}:${line}: ${finding}`);
  };

  const auditsGraphMutationBoundary = path.includes('/features/application/editorMutation/');
  const auditsNodeDetailPanel = path.replace(/\\/g, '/').endsWith(nodeDetailPanelSource);
  const visit = (node: ts.Node): void => {
    if (auditsNodeDetailPanel) {
      const finding = nodeDetailScopedLookupFinding(node, checker, staticString);
      if (finding) report(node, finding);
    }
    if (auditsGraphMutationBoundary && ts.isCallExpression(node)) {
      const expression = unwrapExpression(node.expression);
      if (ts.isPropertyAccessExpression(expression)
        && expression.name.text === 'setState'
        && expressionTargetsGraphDataStore(expression.expression, checker)) {
        report(node, 'graph mutation bypasses the application boundary');
      }
    }

    if (ts.isCallExpression(node)) {
      const projectsRawSignature = node.arguments.some((argument) =>
        isRawExpression(argument));
      if (path !== functionPinSource
        && projectsRawSignature
        && (callTargets(node, checker, functionPinSource, 'createDataSignaturePin')
          || callReturnsFunctionSignaturePin(node, checker))) {
        report(node, 'function signature-to-pin mapping');
        if (node.arguments.some((argument) => staticString(argument) === 'Result')) {
          report(node, 'fixed function output Result');
        }
      }
      if (callTargets(node, checker, dataTypeSource, 'dataTypeFromDisplayString')
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
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return [...new Set(offenders)];
}

function sourceOffendersFromSource(path: string, source: string): string[] {
  const { checker, sourceFile } = createInMemoryProgram(path, source);
  return sourceOffendersFromSourceFile(path, sourceFile, checker);
}

function sourceOffendersFromFixture(sources: Record<string, string>): string[] {
  const program = createFixtureProgram(sources);
  const checker = program.getTypeChecker();
  return Object.keys(sources).flatMap((path) => {
    const sourceFile = program.getSourceFile(resolve(path));
    if (!sourceFile) throw new Error(`fixture source '${path}' was not created`);
    return sourceOffendersFromSourceFile(path, sourceFile, checker);
  });
}

function productionSourceOffenders(paths: string[]): string[] {
  const program = ts.createProgram(paths.map((path) => resolve(path)), parsedConfig.options);
  const checker = program.getTypeChecker();
  return paths.flatMap((path) => {
    const sourceFile = program.getSourceFile(resolve(path));
    if (!sourceFile) throw new Error(`production source '${path}' was not created`);
    return sourceOffendersFromSourceFile(path, sourceFile, checker);
  });
}

describe('frontend stable node identity architecture audit behavior', () => {
  it.each([
    'const descriptor = { resourcePath: path, createArgs: args, nodeTypeId: typeId };',
    'function makeDescriptor() { return { createArgs, nodeTypeId: id, resourcePath: path }; }',
  ])('rejects resource-bound descriptor construction regardless of field order', (source) => {
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
  }, 15_000);
});
