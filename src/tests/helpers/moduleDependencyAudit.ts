import { existsSync, readFileSync, statSync } from 'node:fs';
import { dirname, posix, relative, resolve } from 'node:path';
import * as ts from 'typescript/unstable/ast';
import { SymbolFlags, type Symbol as TypeScriptSymbol } from 'typescript/unstable/sync';
import {
  normalizeTypeScriptPath,
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
  type TypeScriptAuditProject,
} from './typescriptAudit';

export interface ArchitectureSource {
  path: string;
  source: string;
}

export type ModuleDependencyMode = 'runtime' | 'type-only';

export type ModuleDependencyKind =
  | 'static-import'
  | 'side-effect-import'
  | 're-export'
  | 'dynamic-import'
  | 'import-type'
  | 'require'
  | 'import-equals'
  | 'export-assignment';

export type FrontendDependencyMode = ModuleDependencyMode | 'build-style';

export type ModuleDependencyResourceKind = 'module' | 'stylesheet';

export interface ExternalDependencyOrigin {
  readonly packageName: string;
  readonly canonicalSubpath: string | null;
  readonly resourceKind: ModuleDependencyResourceKind;
}

export interface RepositoryAssetDependencyOrigin {
  readonly repositoryRelativeAssetPath: string;
  readonly resourceKind: 'stylesheet';
}

export type StylesheetDependencyOrigin =
  | { readonly kind: 'repository-asset'; readonly asset: RepositoryAssetDependencyOrigin }
  | { readonly kind: 'external'; readonly dependency: ExternalDependencyOrigin };

export type ModuleDependencyOrigin =
  | { readonly kind: 'repository-module'; readonly declarationTarget: string }
  | StylesheetDependencyOrigin;

export interface ModuleDependencyLocation {
  line: number;
  column: number;
}

export interface ModuleDependency {
  kind: ModuleDependencyKind;
  mode: ModuleDependencyMode;
  specifier: string | null;
  location: ModuleDependencyLocation;
}

export interface ResolvedModuleDependency extends ModuleDependency {
  repositoryRelativeSourceFile: string;
  fullyQualifiedOwner: string;
  mode: ModuleDependencyMode;
  origin: ModuleDependencyOrigin;
  canonicalOriginTarget: string;
  importedSymbol: string | null;
  writtenModuleSpecifier: string;
  symbolDeclarationTarget: string | null;
}

export type ModuleDependencyResolutionErrorKind =
  | 'nonliteral-module-specifier'
  | 'invalid-external-specifier'
  | 'invalid-repository-module-specifier'
  | 'unresolved-module-dependency';

export class ModuleDependencyResolutionError extends Error {
  constructor(
    readonly kind: ModuleDependencyResolutionErrorKind,
    readonly sourceFile: string,
    readonly writtenSpecifier: string | null,
    readonly line: number,
    readonly column: number,
  ) {
    super(`${kind}: ${sourceFile}:${line}:${column}`);
    this.name = 'ModuleDependencyResolutionError';
  }
}

interface CollectedModuleDependency extends ModuleDependency {
  readonly node: ts.Node;
  readonly moduleSpecifierNode: ts.Node | null;
  readonly importedSymbolNode: ts.Node | null;
  readonly importedExportName: string | null;
}


function literalText(node: ts.Expression | undefined): string | null {
  return node && (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node))
    ? node.text
    : null;
}

function exportMode(node: ts.ExportDeclaration): ModuleDependencyMode {
  if (node.isTypeOnly) return 'type-only';
  if (node.exportClause && ts.isNamedExports(node.exportClause)) {
    return node.exportClause.elements.length > 0
      && node.exportClause.elements.every((element) => element.isTypeOnly)
      ? 'type-only'
      : 'runtime';
  }
  return 'runtime';
}

function importTypeSpecifier(node: ts.ImportTypeNode): ts.Expression | undefined {
  if (!ts.isLiteralTypeNode(node.argument)) return undefined;
  const literal = node.argument.literal;
  return ts.isStringLiteral(literal) || ts.isNoSubstitutionTemplateLiteral(literal)
    ? literal
    : undefined;
}

function moduleCallSpecifier(node: ts.Expression): {
  kind: 'dynamic-import' | 'require';
  specifier: string | null;
} | null {
  if (!ts.isCallExpression(node)) return null;
  const isDynamicImport = node.expression.kind === ts.SyntaxKind.ImportKeyword;
  const isRequire = ts.isIdentifier(node.expression)
    && node.expression.text === 'require'
    && node.arguments.length === 1;
  if (!isDynamicImport && !isRequire) return null;
  return {
    kind: isDynamicImport ? 'dynamic-import' : 'require',
    specifier: literalText(node.arguments[0]),
  };
}

function collectModuleDependencies(sourceFile: ts.SourceFile): CollectedModuleDependency[] {
  const dependencies: CollectedModuleDependency[] = [];
  const location = (node: ts.Node): ModuleDependencyLocation => {
    const position = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
    return { line: position.line + 1, column: position.character + 1 };
  };
  const add = (
    kind: ModuleDependencyKind,
    mode: ModuleDependencyMode,
    specifier: string | null,
    node: ts.Node,
    moduleSpecifierNode: ts.Node | null,
    importedSymbolNode: ts.Node | null = null,
    importedExportName: string | null = null,
  ): void => {
    dependencies.push({
      kind,
      mode,
      specifier,
      location: location(node),
      node,
      moduleSpecifierNode,
      importedSymbolNode,
      importedExportName,
    });
  };

  const visit = (node: ts.Node): void => {
    if (ts.isImportDeclaration(node)) {
      const specifier = literalText(node.moduleSpecifier);
      const clause = node.importClause;
      if (!clause) {
        add('side-effect-import', 'runtime', specifier, node, node.moduleSpecifier);
        return;
      }
      const clauseIsTypeOnly = clause.phaseModifier === ts.SyntaxKind.TypeKeyword;
      if (clause.name) {
        add(
          'static-import',
          clauseIsTypeOnly ? 'type-only' : 'runtime',
          specifier,
          node,
          node.moduleSpecifier,
          clause.name,
          'default',
        );
      }
      if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) {
        add(
          'static-import',
          clauseIsTypeOnly ? 'type-only' : 'runtime',
          specifier,
          node,
          node.moduleSpecifier,
        );
      }
      if (clause.namedBindings && ts.isNamedImports(clause.namedBindings)) {
        if (clause.namedBindings.elements.length === 0) {
          add(
            'static-import',
            clauseIsTypeOnly ? 'type-only' : 'runtime',
            specifier,
            node,
            node.moduleSpecifier,
          );
        }
        for (const element of clause.namedBindings.elements) {
          add(
            'static-import',
            clauseIsTypeOnly || element.isTypeOnly ? 'type-only' : 'runtime',
            specifier,
            element,
            node.moduleSpecifier,
            element.name,
            (element.propertyName ?? element.name).text,
          );
        }
      }
      return;
    }
    if (ts.isExportDeclaration(node)) {
      if (!node.moduleSpecifier) return;
      const specifier = literalText(node.moduleSpecifier);
      if (node.exportClause && ts.isNamedExports(node.exportClause)) {
        if (node.exportClause.elements.length === 0) {
          add(
            're-export',
            node.isTypeOnly ? 'type-only' : 'runtime',
            specifier,
            node,
            node.moduleSpecifier,
          );
        }
        for (const element of node.exportClause.elements) {
          add(
            're-export',
            node.isTypeOnly || element.isTypeOnly ? 'type-only' : 'runtime',
            specifier,
            element,
            node.moduleSpecifier,
            element.name,
            (element.propertyName ?? element.name).text,
          );
        }
      } else {
        add('re-export', exportMode(node), specifier, node, node.moduleSpecifier);
      }
      return;
    }
    if (ts.isImportEqualsDeclaration(node)
      && ts.isExternalModuleReference(node.moduleReference)) {
      add(
        'import-equals',
        node.isTypeOnly ? 'type-only' : 'runtime',
        literalText(node.moduleReference.expression),
        node,
        node.moduleReference.expression ?? null,
      );
      return;
    }
    if (ts.isImportTypeNode(node)) {
      const specifierNode = importTypeSpecifier(node);
      add(
        'import-type',
        'type-only',
        literalText(specifierNode),
        node,
        specifierNode ?? null,
        node.qualifier ?? null,
        node.qualifier
          ? node.qualifier.getText(sourceFile).split('.').slice(-1)[0] ?? null
          : null,
      );
      node.forEachChild(visit);
      return;
    }
    if (ts.isExportAssignment(node)) {
      const dependency = moduleCallSpecifier(node.expression);
      if (dependency) {
        const call = ts.isCallExpression(node.expression) ? node.expression : null;
        add(
          'export-assignment',
          'runtime',
          dependency.specifier,
          node,
          call?.arguments[0] ?? null,
        );
        call?.forEachChild(visit);
        return;
      }
    }
    if (ts.isCallExpression(node)) {
      const dependency = moduleCallSpecifier(node);
      if (dependency) {
        add(
          dependency.kind,
          'runtime',
          dependency.specifier,
          node,
          node.arguments[0] ?? null,
        );
        node.forEachChild(visit);
        return;
      }
    }
    node.forEachChild(visit);
  };

  visit(sourceFile);
  return dependencies;
}

function withAuditSourceFile<T>(
  path: string,
  source: string,
  callback: (sourceFile: ts.SourceFile) => T,
): T {
  const absolutePath = resolve(path);
  if (existsSync(absolutePath)
    && statSync(absolutePath).isFile()
    && readFileSync(absolutePath, 'utf8') === source) {
    return withProductionTypeScriptProject(({ sourceFile }) => callback(sourceFile(path)));
  }
  return withIsolatedTypeScriptProject(
    { [path]: source },
    ({ sourceFile }) => callback(sourceFile(path)),
  );
}

function repositoryRelativeDeclarationPath(
  context: TypeScriptAuditProject,
  fileName: string,
): string | null {
  const normalized = normalizeTypeScriptPath(resolve(fileName));
  const nodeModulesMarker = '/node_modules/';
  const nodeModulesIndex = normalized.lastIndexOf(nodeModulesMarker);
  if (nodeModulesIndex >= 0) return normalized.slice(nodeModulesIndex + 1);
  const sourceRelative = normalizeTypeScriptPath(relative(context.sourceRoot, normalized));
  if (sourceRelative.startsWith('../') || sourceRelative.includes(':/')) return null;
  return sourceRelative.startsWith('src/') ? sourceRelative : null;
}

function resolvedSymbol(
  context: TypeScriptAuditProject,
  node: ts.Node | null,
): TypeScriptSymbol | null {
  if (!node) return null;
  const symbol = context.checker.getSymbolAtLocation(node);
  if (!symbol) return null;
  if ((symbol.flags & SymbolFlags.Alias) === 0) return symbol;
  const aliased = context.checker.getAliasedSymbol(symbol);
  return context.checker.isUnknownSymbol(aliased) ? null : aliased;
}

function resolveAlias(
  context: TypeScriptAuditProject,
  symbol: TypeScriptSymbol | undefined,
): TypeScriptSymbol | null {
  if (!symbol) return null;
  if ((symbol.flags & SymbolFlags.Alias) === 0) return symbol;
  const aliased = context.checker.getAliasedSymbol(symbol);
  return context.checker.isUnknownSymbol(aliased) ? null : aliased;
}

function dependencySymbol(
  context: TypeScriptAuditProject,
  dependency: CollectedModuleDependency,
): TypeScriptSymbol | null {
  const direct = resolvedSymbol(
    context,
    dependency.importedSymbolNode ?? dependency.moduleSpecifierNode,
  );
  if (direct || dependency.importedExportName === null) return direct;
  const moduleSymbol = resolvedSymbol(context, dependency.moduleSpecifierNode);
  if (!moduleSymbol) return null;
  return resolveAlias(
    context,
    context.checker.getMemberInModuleExports(moduleSymbol, dependency.importedExportName),
  );
}

function symbolDeclaration(
  context: TypeScriptAuditProject,
  symbol: TypeScriptSymbol | null,
): { path: string; symbol: string | null } | null {
  if (!symbol) return null;
  const declarations = symbol.declarations
    .map((handle) => handle.resolve(context.project))
    .filter((node): node is ts.Node => node !== undefined)
    .map((node) => repositoryRelativeDeclarationPath(context, node.getSourceFile().fileName))
    .filter((path): path is string => path !== null)
    .sort();
  const path = declarations[0];
  if (!path) return null;
  return { path, symbol: symbol.name.startsWith('"') ? null : symbol.name };
}

function hasForbiddenEncodedSeparator(specifier: string): boolean {
  const lower = specifier.toLowerCase();
  return lower.includes('%2f') || lower.includes('%5c');
}

export function parseExternalDependencySpecifier(
  specifier: string,
  resourceKind: ModuleDependencyResourceKind,
): ExternalDependencyOrigin | null {
  if (!specifier
    || specifier.startsWith('.')
    || specifier.startsWith('/')
    || specifier.includes('\\')
    || specifier.includes('?')
    || specifier.includes('#')
    || hasForbiddenEncodedSeparator(specifier)) return null;
  const segments = specifier.split('/');
  if (segments.some((segment) => !segment || segment === '.' || segment === '..')) return null;
  const packageSegmentCount = segments[0].startsWith('@') ? 2 : 1;
  if (segments[0] === '@' || segments.length < packageSegmentCount) return null;
  const packageName = segments.slice(0, packageSegmentCount).join('/');
  const subpath = segments.slice(packageSegmentCount);
  return {
    packageName,
    canonicalSubpath: subpath.length > 0 ? subpath.join('::') : null,
    resourceKind,
  };
}

function repositoryStylesheetTarget(importerPath: string, specifier: string): string | null {
  if (!specifier.startsWith('./')
    || specifier.includes('\\')
    || specifier.includes('?')
    || specifier.includes('#')
    || hasForbiddenEncodedSeparator(specifier)
    || specifier.split('/').includes('..')) return null;
  const target = posix.normalize(posix.join(posix.dirname(importerPath), specifier));
  return target.startsWith('src/') && target.endsWith('.css') ? target : null;
}

function repositoryModuleTarget(
  context: TypeScriptAuditProject,
  importerPath: string,
  specifier: string,
): string | null {
  if (!specifier.startsWith('.') && !specifier.startsWith('@/')) return null;
  const base = specifier.startsWith('@/')
    ? posix.join('src', specifier.slice(2))
    : posix.join(posix.dirname(importerPath), specifier);
  const candidates = [
    base,
    `${base}.ts`,
    `${base}.tsx`,
    posix.join(base, 'index.ts'),
    posix.join(base, 'index.tsx'),
  ];
  const programPaths = new Set(
    context.project.program.getSourceFileNames()
      .map((path) => repositoryRelativeDeclarationPath(context, path))
      .filter((path): path is string => path !== null),
  );
  return candidates.find((candidate) => programPaths.has(candidate)) ?? null;
}

function resolutionFailure(
  kind: ModuleDependencyResolutionErrorKind,
  sourceFile: string,
  dependency: CollectedModuleDependency,
): ModuleDependencyResolutionError {
  return new ModuleDependencyResolutionError(
    kind,
    sourceFile,
    dependency.specifier,
    dependency.location.line,
    dependency.location.column,
  );
}

function stylesheetModuleDependency(
  source: ArchitectureSource,
  dependency: CollectedModuleDependency,
): ResolvedModuleDependency | null {
  const specifier = dependency.specifier;
  if (dependency.kind !== 'side-effect-import' || specifier === null || !specifier.endsWith('.css')) {
    return null;
  }
  const external = parseExternalDependencySpecifier(specifier, 'stylesheet');
  const assetPath = external === null
    ? repositoryStylesheetTarget(source.path, specifier)
    : null;
  if (!external && !assetPath) {
    throw resolutionFailure('invalid-repository-module-specifier', source.path, dependency);
  }
  if (external && !existsSync(resolve('node_modules', ...external.packageName.split('/')))) {
    throw resolutionFailure('unresolved-module-dependency', source.path, dependency);
  }
  const origin: StylesheetDependencyOrigin = external
    ? { kind: 'external', dependency: external }
    : {
      kind: 'repository-asset',
      asset: { repositoryRelativeAssetPath: assetPath!, resourceKind: 'stylesheet' },
    };
  const canonicalOriginTarget = origin.kind === 'external'
    ? `external:${origin.dependency.packageName}${origin.dependency.canonicalSubpath === null
      ? ''
      : `::${origin.dependency.canonicalSubpath}`}`
    : `repository-asset:${origin.asset.repositoryRelativeAssetPath}`;
  return {
    kind: dependency.kind,
    mode: dependency.mode,
    specifier,
    location: dependency.location,
    repositoryRelativeSourceFile: source.path,
    fullyQualifiedOwner: `${source.path}::<module>`,
    origin,
    canonicalOriginTarget,
    importedSymbol: null,
    writtenModuleSpecifier: specifier,
    symbolDeclarationTarget: null,
  };
}

export function resolvedModuleDependencies(
  context: TypeScriptAuditProject,
  source: ArchitectureSource,
): readonly ResolvedModuleDependency[] {
  const sourceFile = context.sourceFile(source.path);
  return collectModuleDependencies(sourceFile).map((dependency) => {
    if (dependency.specifier === null) {
      throw resolutionFailure('nonliteral-module-specifier', source.path, dependency);
    }
    const stylesheet = stylesheetModuleDependency(source, dependency);
    if (stylesheet) return stylesheet;

    const external = dependency.specifier.startsWith('.') || dependency.specifier.startsWith('@/')
      ? null
      : parseExternalDependencySpecifier(dependency.specifier, 'module');
    if (!dependency.specifier.startsWith('.')
      && !dependency.specifier.startsWith('@/')
      && external === null) {
      throw resolutionFailure('invalid-external-specifier', source.path, dependency);
    }

    const declaration = symbolDeclaration(context, dependencySymbol(context, dependency));
    const fallbackModuleTarget = repositoryModuleTarget(
      context,
      source.path,
      dependency.specifier,
    );
    if (!declaration && (dependency.importedExportName !== null || fallbackModuleTarget === null)) {
      throw resolutionFailure('unresolved-module-dependency', source.path, dependency);
    }
    const declarationPath = declaration?.path ?? fallbackModuleTarget!;
    const importedSymbol = dependency.importedSymbolNode === null ? null : declaration?.symbol ?? null;
    const symbolDeclarationTarget = importedSymbol === null
      ? declarationPath
      : `${declarationPath}::${importedSymbol}`;

    if (external) {
      const subpath = external.canonicalSubpath === null ? '' : `::${external.canonicalSubpath}`;
      return {
        kind: dependency.kind,
        mode: dependency.mode,
        specifier: dependency.specifier,
        location: dependency.location,
        repositoryRelativeSourceFile: source.path,
        fullyQualifiedOwner: `${source.path}::<module>`,
        origin: { kind: 'external', dependency: external },
        canonicalOriginTarget: `external:${external.packageName}${subpath}`,
        importedSymbol,
        writtenModuleSpecifier: dependency.specifier,
        symbolDeclarationTarget,
      };
    }
    if (!declarationPath.startsWith('src/')) {
      throw resolutionFailure('unresolved-module-dependency', source.path, dependency);
    }
    return {
      kind: dependency.kind,
      mode: dependency.mode,
      specifier: dependency.specifier,
      location: dependency.location,
      repositoryRelativeSourceFile: source.path,
      fullyQualifiedOwner: `${source.path}::<module>`,
      origin: { kind: 'repository-module', declarationTarget: declarationPath },
      canonicalOriginTarget: importedSymbol === null
        ? declarationPath
        : `${declarationPath}::${importedSymbol}`,
      importedSymbol,
      writtenModuleSpecifier: dependency.specifier,
      symbolDeclarationTarget,
    };
  });
}

export function moduleDependencies(path: string, source: string): ModuleDependency[] {
  return withAuditSourceFile(path, source, (sourceFile) => (
    collectModuleDependencies(sourceFile).map(({
      kind,
      mode,
      specifier,
      location,
    }) => ({ kind, mode, specifier, location }))
  ));
}

export function unresolvedRuntimeDependencies(
  path: string,
  source: string,
): ModuleDependency[] {
  return moduleDependencies(path, source).filter((dependency) => (
    dependency.mode === 'runtime' && dependency.specifier === null
  ));
}


export function resolveSourceSpecifier(
  importerPath: string,
  specifier: string,
  sourceRoot = resolve('src'),
  fixtureModules: ReadonlyMap<string, string> = new Map(),
): string | null {
  const absoluteTarget = specifier.startsWith('@/')
    ? resolve(sourceRoot, specifier.slice(2))
    : specifier.startsWith('.')
      ? resolve(dirname(resolve(importerPath)), specifier)
      : null;
  if (absoluteTarget === null) return null;

  const projectPath = (path: string): string => (
    relative(resolve('.'), path).replace(/\\/g, '/')
  );
  const candidates = [
    absoluteTarget,
    `${absoluteTarget}.ts`,
    `${absoluteTarget}.tsx`,
    resolve(absoluteTarget, 'index.ts'),
    resolve(absoluteTarget, 'index.tsx'),
  ];
  const concreteTarget = candidates.find((candidate) => {
    const path = projectPath(candidate);
    return fixtureModules.has(path)
      || (existsSync(candidate) && statSync(candidate).isFile());
  });

  return projectPath(concreteTarget ?? absoluteTarget);
}
