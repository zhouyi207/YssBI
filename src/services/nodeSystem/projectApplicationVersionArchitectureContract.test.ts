import { dirname, relative, resolve } from 'node:path';
import * as ts from 'typescript';
import { describe, expect, it } from 'vitest';

const auditPath = 'src/services/nodeSystem/projectApplicationVersionArchitectureContract.test.ts';
const configPath = resolve('tsconfig.json');
const configFile = ts.readConfigFile(configPath, ts.sys.readFile);
if (configFile.error) {
  throw new Error(ts.flattenDiagnosticMessageText(configFile.error.messageText, '\n'));
}
const parsedConfig = ts.parseJsonConfigFileContent(configFile.config, ts.sys, resolve('.'));

function projectPath(path: string): string {
  return relative(resolve('.'), path).replace(/\\/g, '/');
}

function createFixtureProgram(sources: Record<string, string>): ts.Program {
  const virtualSources = new Map(Object.entries(sources).map(([path, source]) => [
    resolve(path),
    source,
  ]));
  const virtualDirectories = new Set<string>();
  for (const path of virtualSources.keys()) {
    let directory = dirname(path);
    while (!virtualDirectories.has(directory)) {
      virtualDirectories.add(directory);
      const parent = dirname(directory);
      if (parent === directory) break;
      directory = parent;
    }
  }

  const host = ts.createCompilerHost(parsedConfig.options, true);
  const baseDirectoryExists = host.directoryExists?.bind(host);
  const baseFileExists = host.fileExists.bind(host);
  const baseReadFile = host.readFile.bind(host);
  const baseGetSourceFile = host.getSourceFile.bind(host);
  host.directoryExists = (directoryName) =>
    virtualDirectories.has(resolve(directoryName))
    || (baseDirectoryExists?.(directoryName) ?? false);
  host.fileExists = (fileName) => virtualSources.has(resolve(fileName)) || baseFileExists(fileName);
  host.readFile = (fileName) => virtualSources.get(resolve(fileName)) ?? baseReadFile(fileName);
  host.getSourceFile = (fileName, languageVersion, onError, shouldCreateNewSourceFile) => {
    const source = virtualSources.get(resolve(fileName));
    return source === undefined
      ? baseGetSourceFile(fileName, languageVersion, onError, shouldCreateNewSourceFile)
      : ts.createSourceFile(
        fileName,
        source,
        languageVersion,
        true,
        fileName.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
      );
  };
  return ts.createProgram([...virtualSources.keys()], parsedConfig.options, host);
}

function unwrapExpression(node: ts.Expression): ts.Expression {
  if (ts.isParenthesizedExpression(node)
    || ts.isAsExpression(node)
    || ts.isTypeAssertionExpression(node)
    || ts.isNonNullExpression(node)
    || ts.isSatisfiesExpression(node)) {
    return unwrapExpression(node.expression);
  }
  return node;
}

function staticString(
  node: ts.Expression,
  checker: ts.TypeChecker,
  visiting: ReadonlySet<ts.Symbol> = new Set(),
): string | null {
  const expression = unwrapExpression(node);
  if (ts.isStringLiteralLike(expression)) return expression.text;

  const expressionType = checker.getTypeAtLocation(expression);
  if ((expressionType.flags & ts.TypeFlags.StringLiteral) !== 0) {
    return (expressionType as ts.StringLiteralType).value;
  }

  if (ts.isIdentifier(expression)) {
    const symbol = checker.getSymbolAtLocation(expression);
    if (!symbol) return null;
    const resolved = resolvedSymbol(symbol, checker);
    if (visiting.has(resolved)) return null;
    const declaration = resolved.valueDeclaration;
    if (!declaration
      || !ts.isVariableDeclaration(declaration)
      || !declaration.initializer
      || !ts.isVariableDeclarationList(declaration.parent)
      || (declaration.parent.flags & ts.NodeFlags.Const) === 0) return null;
    return staticString(declaration.initializer, checker, new Set([...visiting, resolved]));
  }
  if (ts.isBinaryExpression(expression)
    && expression.operatorToken.kind === ts.SyntaxKind.PlusToken) {
    const left = staticString(expression.left, checker, visiting);
    const right = staticString(expression.right, checker, visiting);
    return left !== null && right !== null ? left + right : null;
  }
  if (ts.isTemplateExpression(expression)) {
    let value = expression.head.text;
    for (const span of expression.templateSpans) {
      const substitution = staticString(span.expression, checker, visiting);
      if (substitution === null) return null;
      value += substitution + span.literal.text;
    }
    return value;
  }
  return null;
}

function declaredPropertyName(
  name: ts.PropertyName | ts.BindingName | undefined,
  checker: ts.TypeChecker,
): string | null {
  if (!name) return null;
  if (ts.isIdentifier(name) || ts.isStringLiteralLike(name)) return name.text;
  if (ts.isComputedPropertyName(name)) return staticString(name.expression, checker);
  return null;
}

const STANDARD_PROPERTY_KEY_API_ARGUMENT = new Map<string, number>([
  ['ObjectConstructor.defineProperty', 1],
  ['ObjectConstructor.getOwnPropertyDescriptor', 1],
  ['ObjectConstructor.hasOwn', 1],
  ['Object.hasOwnProperty.call', 1],
  ['Object.propertyIsEnumerable.call', 1],
  ['Reflect.defineProperty', 1],
  ['Reflect.deleteProperty', 1],
  ['Reflect.get', 1],
  ['Reflect.getOwnPropertyDescriptor', 1],
  ['Reflect.has', 1],
  ['Reflect.set', 1],
]);
const STANDARD_FROM_ENTRIES_API = 'ObjectConstructor.fromEntries';

function standardLibraryApiName(
  symbol: ts.Symbol | undefined,
  checker: ts.TypeChecker,
): string | null {
  if (!symbol) return null;
  const resolved = resolvedSymbol(symbol, checker);
  const declarations = resolved.declarations ?? [];
  if (!declarations.some((declaration) => declaration.getSourceFile().hasNoDefaultLib)) return null;
  return checker.getFullyQualifiedName(resolved);
}

function constVariableInitializer(
  expression: ts.Expression,
  checker: ts.TypeChecker,
  visiting: ReadonlySet<ts.Symbol>,
): { initializer: ts.Expression; visiting: ReadonlySet<ts.Symbol> } | null {
  const unwrapped = unwrapExpression(expression);
  if (!ts.isIdentifier(unwrapped)) return null;
  const symbol = checker.getSymbolAtLocation(unwrapped);
  if (!symbol) return null;
  const resolved = resolvedSymbol(symbol, checker);
  if (visiting.has(resolved)) return null;
  const declaration = resolved.valueDeclaration;
  if (!declaration
    || !ts.isVariableDeclaration(declaration)
    || !declaration.initializer
    || !ts.isVariableDeclarationList(declaration.parent)
    || (declaration.parent.flags & ts.NodeFlags.Const) === 0) return null;
  return {
    initializer: declaration.initializer,
    visiting: new Set([...visiting, resolved]),
  };
}

function standardApiFromBindingElement(
  declaration: ts.BindingElement,
  checker: ts.TypeChecker,
): string | null {
  if (!ts.isObjectBindingPattern(declaration.parent)) return null;
  const variable = declaration.parent.parent;
  if (!ts.isVariableDeclaration(variable)
    || !variable.initializer
    || !ts.isVariableDeclarationList(variable.parent)
    || (variable.parent.flags & ts.NodeFlags.Const) === 0) return null;
  const propertyName = declaredPropertyName(declaration.propertyName ?? declaration.name, checker);
  if (!propertyName) return null;
  const receiverType = checker.getTypeAtLocation(variable.initializer);
  return standardLibraryApiName(checker.getPropertyOfType(receiverType, propertyName), checker);
}

function standardApiFromExpression(
  node: ts.Expression,
  checker: ts.TypeChecker,
  visiting: ReadonlySet<ts.Symbol> = new Set(),
): string | null {
  const expression = unwrapExpression(node);
  if (ts.isPropertyAccessExpression(expression)) {
    const direct = standardLibraryApiName(checker.getSymbolAtLocation(expression.name), checker);
    if (direct === 'Function.call' && ts.isPropertyAccessExpression(expression.expression)) {
      const receiver = standardLibraryApiName(
        checker.getSymbolAtLocation(expression.expression.name),
        checker,
      );
      if (receiver === 'Object.hasOwnProperty' || receiver === 'Object.propertyIsEnumerable') {
        return `${receiver}.call`;
      }
    }
    if (direct) return direct;
  }

  if (ts.isIdentifier(expression)) {
    const symbol = checker.getSymbolAtLocation(expression);
    const resolved = symbol && resolvedSymbol(symbol, checker);
    if (resolved && !visiting.has(resolved)) {
      const declaration = resolved.valueDeclaration;
      if (declaration && ts.isBindingElement(declaration)) {
        const api = standardApiFromBindingElement(declaration, checker);
        if (api) return api;
      }
    }
  }

  const flow = constVariableInitializer(expression, checker, visiting);
  return flow
    ? standardApiFromExpression(flow.initializer, checker, flow.visiting)
    : null;
}

function standardApiForCall(call: ts.CallExpression, checker: ts.TypeChecker): string | null {
  const flowed = standardApiFromExpression(call.expression, checker);
  if (flowed) return flowed;
  const declaration = checker.getResolvedSignature(call)?.declaration;
  return declaration && 'name' in declaration && declaration.name
    ? standardLibraryApiName(checker.getSymbolAtLocation(declaration.name), checker)
    : null;
}

function arrayLiteral(
  node: ts.Expression,
  checker: ts.TypeChecker,
  visiting: ReadonlySet<ts.Symbol> = new Set(),
): ts.ArrayLiteralExpression | null {
  const expression = unwrapExpression(node);
  if (ts.isArrayLiteralExpression(expression)) return expression;
  const flow = constVariableInitializer(expression, checker, visiting);
  return flow ? arrayLiteral(flow.initializer, checker, flow.visiting) : null;
}

function fromEntriesContainsProjectApplicationVersion(
  call: ts.CallExpression,
  checker: ts.TypeChecker,
): boolean {
  if (standardApiForCall(call, checker) !== STANDARD_FROM_ENTRIES_API) return false;
  const entries = call.arguments[0] && arrayLiteral(call.arguments[0], checker);
  if (!entries) return false;
  return entries.elements.some((entry) => {
    if (!ts.isExpression(entry)) return false;
    const tuple = arrayLiteral(entry, checker);
    const key = tuple?.elements[0];
    return key !== undefined
      && ts.isExpression(key)
      && staticString(key, checker) === 'appVersion';
  });
}

function propertyKeyApiExposesProjectApplicationVersion(
  call: ts.CallExpression,
  checker: ts.TypeChecker,
): boolean {
  const api = standardApiForCall(call, checker);
  if (!api) return false;
  const keyIndex = STANDARD_PROPERTY_KEY_API_ARGUMENT.get(api);
  const key = keyIndex === undefined ? undefined : call.arguments[keyIndex];
  return key !== undefined && staticString(key, checker) === 'appVersion';
}

function resolvedSymbol(symbol: ts.Symbol, checker: ts.TypeChecker): ts.Symbol {
  return (symbol.flags & ts.SymbolFlags.Alias) !== 0
    ? checker.getAliasedSymbol(symbol)
    : symbol;
}

function typeExposesProjectApplicationVersion(
  type: ts.Type,
  checker: ts.TypeChecker,
  visiting: ReadonlySet<ts.Type> = new Set(),
): boolean {
  if (visiting.has(type)) return false;
  const nextVisiting = new Set([...visiting, type]);

  if (checker.getPropertyOfType(type, 'appVersion')) return true;

  if (type.isUnionOrIntersection()
    && type.types.some((member) =>
      typeExposesProjectApplicationVersion(member, checker, nextVisiting))) return true;

  return [...type.getCallSignatures(), ...type.getConstructSignatures()].some((signature) =>
    typeExposesProjectApplicationVersion(
      checker.getReturnTypeOfSignature(signature),
      checker,
      nextVisiting,
    ));
}

function symbolExposesProjectApplicationVersion(
  symbol: ts.Symbol | undefined,
  location: ts.Node,
  checker: ts.TypeChecker,
): boolean {
  if (!symbol) return false;
  const resolved = resolvedSymbol(symbol, checker);
  const declaration = resolved.valueDeclaration ?? resolved.declarations?.[0] ?? location;
  return [
    checker.getTypeOfSymbolAtLocation(resolved, declaration),
    checker.getDeclaredTypeOfSymbol(resolved),
  ].some((type) => typeExposesProjectApplicationVersion(type, checker));
}

function returnTypeExposesProjectApplicationVersion(
  node: ts.SignatureDeclaration,
  checker: ts.TypeChecker,
): boolean {
  const signature = checker.getSignatureFromDeclaration(node);
  return signature !== undefined
    && typeExposesProjectApplicationVersion(checker.getReturnTypeOfSignature(signature), checker);
}

function projectApplicationVersionOffenders(
  program: ts.Program,
  includedPaths: ReadonlySet<string>,
): string[] {
  const checker = program.getTypeChecker();
  const offenders = new Set<string>();

  for (const sourceFile of program.getSourceFiles()) {
    const path = projectPath(sourceFile.fileName);
    if (!includedPaths.has(path)) continue;

    const report = (node: ts.Node, label: string): void => {
      const line = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
      offenders.add(`${path}:${line}: ${label}`);
    };

    const visit = (node: ts.Node): void => {
      const namedDeclaration = ts.isPropertySignature(node)
        || ts.isPropertyDeclaration(node)
        || ts.isPropertyAssignment(node)
        || ts.isShorthandPropertyAssignment(node)
        || ts.isMethodSignature(node)
        || ts.isMethodDeclaration(node)
        || ts.isGetAccessorDeclaration(node)
        || ts.isSetAccessorDeclaration(node);
      if (namedDeclaration && declaredPropertyName(node.name, checker) === 'appVersion') {
        report(node, 'project application version property');
      }
      if (ts.isBindingElement(node)
        && declaredPropertyName(node.propertyName ?? node.name, checker) === 'appVersion') {
        report(node, 'project application version property');
      }
      if (ts.isJsxAttribute(node)
        && ts.isIdentifier(node.name)
        && node.name.text === 'appVersion') {
        report(node, 'project application version property');
      }

      if (ts.isPropertyAccessExpression(node) && node.name.text === 'appVersion') {
        report(node, 'project application version member access');
      }
      if (ts.isElementAccessExpression(node)
        && node.argumentExpression
        && staticString(node.argumentExpression, checker) === 'appVersion') {
        report(node, 'project application version member access');
      }

      if ((ts.isInterfaceDeclaration(node) || ts.isClassDeclaration(node))
        && symbolExposesProjectApplicationVersion(
          node.name ? checker.getSymbolAtLocation(node.name) : undefined,
          node,
          checker,
        )) {
        report(node, 'project application version declaration type');
      }
      if (ts.isTypeAliasDeclaration(node)
        && symbolExposesProjectApplicationVersion(
          checker.getSymbolAtLocation(node.name), node, checker,
        )) {
        report(node, 'project application version type alias');
      }
      if (ts.isVariableDeclaration(node)
        && typeExposesProjectApplicationVersion(checker.getTypeAtLocation(node.name), checker)) {
        report(node, 'project application version variable type');
      }
      if (ts.isFunctionLike(node)
        && returnTypeExposesProjectApplicationVersion(node, checker)) {
        report(node, 'project application version return type');
      }
      if (ts.isCallExpression(node)
        && propertyKeyApiExposesProjectApplicationVersion(node, checker)) {
        report(node, 'project application version property-key API');
      }
      if (ts.isCallExpression(node)
        && fromEntriesContainsProjectApplicationVersion(node, checker)) {
        report(node, 'project application version fromEntries key');
      }
      if ((ts.isCallExpression(node) || ts.isNewExpression(node))
        && typeExposesProjectApplicationVersion(checker.getTypeAtLocation(node), checker)) {
        report(node, 'project application version expression type');
      }
      if ((ts.isImportSpecifier(node) || ts.isExportSpecifier(node))
        && symbolExposesProjectApplicationVersion(
          checker.getSymbolAtLocation(node.propertyName ?? node.name),
          node,
          checker,
        )) {
        report(node, 'project application version alias or re-export');
      }
      if (ts.isExportDeclaration(node)
        && !node.exportClause
        && node.moduleSpecifier) {
        const moduleSymbol = checker.getSymbolAtLocation(node.moduleSpecifier);
        if (moduleSymbol && checker.getExportsOfModule(moduleSymbol).some((symbol) =>
          symbolExposesProjectApplicationVersion(symbol, node, checker))) {
          report(node, 'project application version wildcard re-export');
        }
      }

      ts.forEachChild(node, visit);
    };

    visit(sourceFile);
  }

  return [...offenders];
}

function fixtureOffenders(sources: Record<string, string>): string[] {
  const program = createFixtureProgram(sources);
  return projectApplicationVersionOffenders(
    program,
    new Set(Object.keys(sources).map((path) => projectPath(resolve(path)))),
  );
}

function productionOffenders(): string[] {
  const program = ts.createProgram(parsedConfig.fileNames, parsedConfig.options);
  const includedPaths = new Set(parsedConfig.fileNames
    .map(projectPath)
    .filter((path) => path !== auditPath));
  return projectApplicationVersionOffenders(program, includedPaths);
}

describe('project application-version architecture audit behavior', () => {
  it('rejects a direct project metadata property declaration', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/direct.ts': `
        export interface ProjectIndexRow { appVersion: string }
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /direct\.ts:\d+: project application version property/,
    ));
  });

  it.each([
    ['interface', `interface ProjectMetadata extends Record<'appVersion', string> {}`],
    [
      'class',
      `declare const Base: new () => Record<'appVersion', string>; declare class ProjectMetadata extends Base {}`,
    ],
  ])('rejects checker-exposed application metadata through %s inheritance', (_, source) => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/inheritance.ts': source,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /inheritance\.ts:\d+: project application version declaration type/,
    ));
  });

  it.each(['metadata', 'snapshot', 'projectIndex'])(
    'rejects a generic %s builder result that exposes application metadata',
    (owner) => {
      const path = `src/__architecture_fixture__/${owner}Builder.ts`;
      const offenders = fixtureOffenders({
        [path]: `
          declare function field<K extends string, V>(key: K, value: V): Record<K, V>;
          const ${owner} = field('appVersion', '1.0.0');
        `,
      });

      expect(offenders).toContainEqual(expect.stringMatching(
        new RegExp(`${owner}Builder\\.ts:\\d+: project application version expression type`),
      ));
    },
  );

  it.each(['manifest', 'project', 'exportData'])(
    'rejects a synthetic generic %s result outside project source boundaries',
    (binding) => {
      const path = `src/__architecture_fixture__/${binding}Generic.ts`;
      const offenders = fixtureOffenders({
        [path]: `
          declare function field<K extends string, V>(key: K, value: V): Record<K, V>;
          const ${binding} = field('appVersion', '1.0.0');
        `,
      });

      expect(offenders).toContainEqual(expect.stringMatching(
        new RegExp(`${binding}Generic\\.ts:\\d+: project application version expression type`),
      ));
    },
  );

  it.each(['ProjectMetadata', 'ProjectData', 'ProjectIndexRow', 'ProjectManifest'])(
    'rejects authoritative type %s regardless of source path',
    (typeName) => {
      const offenders = fixtureOffenders({
        'src/__architecture_fixture__/authoritative.ts': `
          interface ${typeName} { appVersion: string }
        `,
      });

      expect(offenders).toContainEqual(expect.stringMatching(
        /authoritative\.ts:\d+: project application version property/,
      ));
    },
  );

  it('rejects a variable whose checker type exposes application metadata', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/variable.ts': `
        declare const metadata: Record<'appVersion', string>;
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /variable\.ts:\d+: project application version variable type/,
    ));
  });

  it('rejects a function declaration whose return type exposes application metadata', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/functionReturn.ts': `
        declare function metadata(): Record<'appVersion', string>;
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /functionReturn\.ts:\d+: project application version return type/,
    ));
  });

  it('rejects a project metadata call whose return type exposes application metadata', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/callReturn.ts': `
        declare function loadProjectMetadata(): Record<'appVersion', string>;
        loadProjectMetadata();
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /callReturn\.ts:\d+: project application version expression type/,
    ));
  });

  it('resolves an imported const used as a computed project property', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/key.ts': `export const projectKey = 'appVersion' as const;`,
      'src/__architecture_fixture__/importedConst.ts': `
        import { projectKey as key } from './key';
        interface ProjectMetadata { [key]: string }
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /importedConst\.ts:\d+: project application version property/,
    ));
  });

  it.each([
    ['concatenation', `const key = 'app' + 'Version'; interface ProjectMetadata { [key]: string }`],
    ['template', "const prefix = 'app'; const suffix = 'Version'; interface ProjectMetadata { [`${prefix}${suffix}`]: string }"],
  ])('folds checker-known %s computed project properties', (name, source) => {
    const offenders = fixtureOffenders({
      [`src/__architecture_fixture__/${name}.ts`]: source,
    });

    expect(offenders).toContainEqual(expect.stringContaining(
      `${name}.ts:1: project application version property`,
    ));
  });

  it('rejects a named alias re-export without relying on member access', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/metadata.ts': `
        export interface ProjectMetadata extends Record<'appVersion', string> {}
      `,
      'src/__architecture_fixture__/alias.ts': `
        export { ProjectMetadata as SnapshotMetadata } from './metadata';
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /alias\.ts:\d+: project application version alias or re-export/,
    ));
  });

  it('rejects a wildcard re-export without relying on member access', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/metadata.ts': `
        export interface ProjectMetadata extends Record<'appVersion', string> {}
      `,
      'src/__architecture_fixture__/barrel.ts': `export * from './metadata';`,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /barrel\.ts:\d+: project application version wildcard re-export/,
    ));
  });

  it.each([
    ['interface', `interface ExternalRuntimeInfo { appVersion: string }`],
    ['type literal', `type ExternalRuntimeInfo = { appVersion: string }`],
  ])('rejects a non-project external-runtime %s exposing the reserved name', (_, source) => {
    expect(fixtureOffenders({
      'src/__architecture_fixture__/externalRuntime.ts': source,
    })).toContainEqual(expect.stringMatching(
      /externalRuntime\.ts:\d+: project application version property/,
    ));
  });

  it('rejects an imported external-runtime return exposing the reserved name', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/externalRuntime.ts': `
        export interface ExternalRuntimeInfo { appVersion: string }
        export declare function loadRuntime(): ExternalRuntimeInfo;
      `,
      'src/__architecture_fixture__/runtimeConsumer.ts': `
        import { loadRuntime } from './externalRuntime';
        const runtime = loadRuntime();
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /runtimeConsumer\.ts:\d+: project application version expression type/,
    ));
  });

  it('rejects construction of an external type exposing the reserved name', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/runtimeConstructor.ts': `
        declare class ExternalRuntimeInfo { appVersion: string }
        new ExternalRuntimeInfo();
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /runtimeConstructor\.ts:\d+: project application version expression type/,
    ));
  });

  it('rejects an imported alias exposing the reserved name', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/externalRuntime.ts': `
        export interface ExternalRuntimeInfo { appVersion: string }
      `,
      'src/__architecture_fixture__/runtimeAlias.ts': `
        import { ExternalRuntimeInfo as RuntimeInfo } from './externalRuntime';
        declare const runtime: RuntimeInfo;
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /runtimeAlias\.ts:\d+: project application version alias or re-export/,
    ));
  });

  it('rejects renamed destructuring of the reserved property', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/destructuring.ts': `
        declare const runtime: Record<string, unknown>;
        const { appVersion: release } = runtime;
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /destructuring\.ts:\d+: project application version property/,
    ));
  });

  it('rejects the reserved JSX attribute', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/attribute.tsx': `
        declare function Runtime(props: Record<string, unknown>): unknown;
        const view = <Runtime appVersion="1.0.0" />;
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /attribute\.tsx:\d+: project application version property/,
    ));
  });

  it('rejects Object.defineProperty with a static reserved key', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/objectDefineProperty.ts': `
        const runtime = {};
        Object.defineProperty(runtime, 'appVersion', { value: '1.0.0' });
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /objectDefineProperty\.ts:\d+: project application version property-key API/,
    ));
  });

  it('rejects Reflect.defineProperty with a static reserved key', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/reflectDefineProperty.ts': `
        const runtime = {};
        Reflect.defineProperty(runtime, 'appVersion', { value: '1.0.0' });
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /reflectDefineProperty\.ts:\d+: project application version property-key API/,
    ));
  });

  it('rejects Reflect.set with a static reserved key', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/reflectSet.ts': `
        const runtime = {};
        Reflect.set(runtime, 'appVersion', '1.0.0');
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /reflectSet\.ts:\d+: project application version property-key API/,
    ));
  });

  it('rejects Reflect.get with a static reserved key', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/reflectGet.ts': `
        const runtime = {};
        Reflect.get(runtime, 'appVersion');
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /reflectGet\.ts:\d+: project application version property-key API/,
    ));
  });

  it('rejects Object.fromEntries with a static reserved tuple key', () => {
    const offenders = fixtureOffenders({
      'src/__architecture_fixture__/fromEntries.ts': `
        const runtime = Object.fromEntries([['appVersion', '1.0.0'] as const]);
      `,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /fromEntries\.ts:\d+: project application version fromEntries key/,
    ));
  });

  it.each([
    ['const property alias', `
      const define = Object.defineProperty;
      define({}, 'appVersion', { value: '1.0.0' });
    `],
    ['destructured Object alias', `
      const { defineProperty: define } = Object;
      define({}, 'appVersion', { value: '1.0.0' });
    `],
    ['destructured Reflect alias', `
      const { get: read } = Reflect;
      read({}, 'appVersion');
    `],
    ['Reflect namespace alias', `
      const R = Reflect;
      R.set({}, 'appVersion', '1.0.0');
    `],
  ])('rejects a reserved key through a %s', (name, source) => {
    const offenders = fixtureOffenders({
      [`src/__architecture_fixture__/${name.replace(/ /g, '-')}.ts`]: source,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /project application version property-key API/,
    ));
  });

  it.each([
    ['const entries', `
      const entries = [['appVersion', '1.0.0'] as const] as const;
      Object.fromEntries(entries);
    `],
    ['const tuple', `
      const entry = ['appVersion', '1.0.0'] as const;
      const entries = [entry] as const;
      Object.fromEntries(entries);
    `],
    ['aliased fromEntries', `
      const fromEntries = Object.fromEntries;
      const entries = [['appVersion', '1.0.0'] as const] as const;
      fromEntries(entries);
    `],
    ['destructured fromEntries', `
      const { fromEntries: build } = Object;
      const entries = [['appVersion', '1.0.0'] as const] as const;
      build(entries);
    `],
  ])('rejects a reserved key through %s const flow', (name, source) => {
    const offenders = fixtureOffenders({
      [`src/__architecture_fixture__/${name.replace(/ /g, '-')}.ts`]: source,
    });

    expect(offenders).toContainEqual(expect.stringMatching(
      /project application version fromEntries key/,
    ));
  });

  it('allows shadowed Object and Reflect property APIs', () => {
    expect(fixtureOffenders({
      'src/__architecture_fixture__/shadowedPropertyApis.ts': `
        export {};
        const Object = {
          defineProperty(_target: object, _key: string, _descriptor: object) {},
          fromEntries(_entries: readonly unknown[]) { return {}; },
        };
        const Reflect = {
          get(_target: object, _key: string) { return undefined; },
          set(_target: object, _key: string, _value: unknown) { return true; },
        };
        Object.defineProperty({}, 'appVersion', { value: '1.0.0' });
        Object.fromEntries([['appVersion', '1.0.0']]);
        Reflect.get({}, 'appVersion');
        Reflect.set({}, 'appVersion', '1.0.0');
      `,
    })).toEqual([]);
  });

  it('allows dynamic join and runtime-version property-key decoys', () => {
    expect(fixtureOffenders({
      'src/__architecture_fixture__/propertyKeyDecoys.ts': `
        const runtime = {};
        const dynamicKey = ['app', 'Version'].join('');
        Object.defineProperty(runtime, dynamicKey, { value: '1.0.0' });
        Reflect.defineProperty(runtime, 'runtimeVersion', { value: '1.0.0' });
        Reflect.set(runtime, dynamicKey, '1.0.0');
        Reflect.get(runtime, 'runtimeVersion');
        const define = Object.defineProperty;
        const fromEntries = Object.fromEntries;
        define(runtime, dynamicKey, { value: '1.0.0' });
        Object.fromEntries([[dynamicKey, '1.0.0'], ['runtimeVersion', '1.0.0']]);
        fromEntries([[dynamicKey, '1.0.0']]);
      `,
    })).toEqual([]);
  });

  it('allows schema, runtime, revision, protocol, dynamic keys, and product-version decoys', () => {
    expect(fixtureOffenders({
      'src/__architecture_fixture__/decoys.ts': `
        export interface ProjectManifest { schemaVersion: number }
        export interface ExternalRuntimeInfo { runtimeVersion: string; version: string }
        export interface Resource { resourceRevision: number; publicationRevision: number }
        export interface Protocol { wireVersion: number; semanticsVersion: number }
        export const APP_VERSION = '9.8.7';
        declare const index: Record<string, unknown>;
        index[['app', 'Version'].join('')] = APP_VERSION;
      `,
    })).toEqual([]);
  });
});

describe('project application-version architecture', () => {
  it('keeps project production and ordinary fixture sources free of application metadata', () => {
    expect(productionOffenders()).toEqual([]);
  }, 30_000);
});
