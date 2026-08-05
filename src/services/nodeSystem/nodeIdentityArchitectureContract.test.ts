import { readFileSync, readdirSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import * as ts from 'typescript';
import { describe, expect, it } from 'vitest';

const sourceRoot = resolve('src');
const auditPath = 'src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts';

function literal(parts: readonly string[]): string {
  return parts.join('');
}

const forbiddenTokens = [
  literal(['Functions:', 'Call Function']),
  literal(['Variables:', 'Get Variable']),
  literal(['Variables:', 'Set Variable']),
  literal(['Data:', 'Get DataFrame']),
  literal(['resolveEffective', 'Definition']),
  literal(['signatureTo', 'PinSlots']),
  literal(['defaultFunction', 'Signature']),
  literal(['@/features/domain/', 'nodeDefinition']),
];

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

function sourceOffendersFromSource(path: string, source: string): string[] {
  const { checker, sourceFile } = createInMemoryProgram(path, source);
  const staticString = createStaticEvaluator(sourceFile, checker);
  const offenders: string[] = [];
  const report = (node: ts.Node, finding: string) => {
    const line = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
    offenders.push(`${path}:${line}: ${finding}`);
  };

  const visit = (node: ts.Node): void => {
    if (ts.isExpression(node)) {
      const value = staticString(node);
      if (value !== null && forbiddenTokens.includes(value)) report(node, value);
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

describe('frontend stable node identity architecture audit behavior', () => {
  it.each([
    ["const value = 'Functions:' + 'Call Function';", 'Functions:Call Function'],
    ["const value = ['Variables:', 'Get Variable'].join('');", 'Variables:Get Variable'],
    [
      "const namespace = 'Functions:'; const name = 'Call Function'; const value = namespace + name;",
      'Functions:Call Function',
    ],
    [
      "const parts = ['Variables:', 'Get Variable'] as const; const value = parts.join('');",
      'Variables:Get Variable',
    ],
    [
      "const namespace = 'Functions:'; const alias = namespace; const name = 'Call Function'; const value = `${alias}${name}`;",
      'Functions:Call Function',
    ],
  ])('rejects statically evaluable legacy identity source: %s', (source, token) => {
    expect(sourceOffendersFromSource('src/features/example.ts', source))
      .toContainEqual(expect.stringContaining(token));
  });

  it('resolves same-named const bindings independently across function scopes', () => {
    const source = `
      function offender() {
        const namespace = 'Functions:';
        const name = 'Call Function';
        return namespace + name;
      }
      function unrelated() {
        const namespace = 'Other:';
        const name = 'Other Name';
        return namespace + name;
      }
    `;
    expect(sourceOffendersFromSource('src/features/example.ts', source))
      .toContainEqual(expect.stringContaining('Functions:Call Function'));
  });

  it.each([
    `function safe() { let namespace = 'Safe:'; const name = 'Call Function'; return namespace + name; }
     function unrelated() { const namespace = 'Functions:'; return namespace; }`,
    `function safe() { var namespace = 'Safe:'; const name = 'Call Function'; return namespace + name; }
     function unrelated() { const namespace = 'Functions:'; return namespace; }`,
    `function safe(namespace: string) { const name = 'Call Function'; return namespace + name; }
     function unrelated() { const namespace = 'Functions:'; return namespace; }`,
    `import { namespace } from './identity';
     const name = 'Call Function';
     namespace + name;
     function unrelated() { const namespace = 'Functions:'; return namespace; }`,
  ])('does not bind a runtime lexical symbol to another scope const: %s', (source) => {
    expect(sourceOffendersFromSource('src/features/example.ts', source)).toEqual([]);
  });

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

  it.each([
    "let namespace = 'Functions:'; const name = 'Call Function'; namespace + name;",
    "var namespace = 'Functions:'; const name = 'Call Function'; namespace + name;",
    "const namespace = getNamespace(); const name = 'Call Function'; namespace + name;",
    "const namespace = config.namespace; const name = 'Call Function'; namespace + name;",
    "import { namespace } from './identity'; const name = 'Call Function'; namespace + name;",
    "const left = right; const right = left; left + 'Call Function';",
  ])('does not evaluate mutable, executable, external, property, or cyclic sources: %s', (source) => {
    expect(sourceOffendersFromSource('src/features/example.ts', source)).toEqual([]);
  });

  it('excludes only the exact tests/fixtures directory segment', () => {
    expect(isFixtureDirectory('tests/fixtures')).toBe(true);
    expect(isFixtureDirectory('tests/fixtures/catalog.json')).toBe(true);
    expect(isFixtureDirectory('tests/fixturesProduction')).toBe(false);
    expect(isFixtureDirectory('tests/fixturesProduction/offender.ts')).toBe(false);
  });
});

describe('frontend stable node identity architecture', () => {
  it('keeps production sources free of legacy identities and descriptor synthesis', () => {
    const offenders = productionFiles().flatMap((path) =>
      sourceOffendersFromSource(path, readFileSync(resolve(path), 'utf8')));

    expect(offenders).toEqual([]);
  });
});
