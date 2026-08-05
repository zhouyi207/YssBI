import { existsSync, statSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import * as ts from 'typescript';

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
  | 'require'
  | 'import-equals'
  | 'export-assignment';

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

function scriptKind(path: string): ts.ScriptKind {
  return path.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
}

function literalText(node: ts.Expression | undefined): string | null {
  return node && ts.isStringLiteralLike(node) ? node.text : null;
}

function importMode(node: ts.ImportDeclaration): ModuleDependencyMode {
  const clause = node.importClause;
  if (!clause) return 'runtime';
  if (clause.isTypeOnly) return 'type-only';
  if (clause.name || !clause.namedBindings || ts.isNamespaceImport(clause.namedBindings)) {
    return 'runtime';
  }
  return clause.namedBindings.elements.length > 0
    && clause.namedBindings.elements.every((element) => element.isTypeOnly)
    ? 'type-only'
    : 'runtime';
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

function moduleCallSpecifier(node: ts.Expression): {
  kind: 'dynamic-import' | 'require';
  specifier: string | null;
} | null {
  if (!ts.isCallExpression(node)) return null;
  const isDynamicImport = node.expression.kind === ts.SyntaxKind.ImportKeyword
    && node.arguments.length >= 1;
  const isRequire = ts.isIdentifier(node.expression)
    && node.expression.text === 'require'
    && node.arguments.length === 1;
  if (!isDynamicImport && !isRequire) return null;
  return {
    kind: isDynamicImport ? 'dynamic-import' : 'require',
    specifier: literalText(node.arguments[0]),
  };
}

export function moduleDependencies(path: string, source: string): ModuleDependency[] {
  const sourceFile = ts.createSourceFile(
    path,
    source,
    ts.ScriptTarget.Latest,
    true,
    scriptKind(path),
  );
  const dependencies: ModuleDependency[] = [];
  const location = (node: ts.Node): ModuleDependencyLocation => {
    const position = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
    return { line: position.line + 1, column: position.character + 1 };
  };
  const add = (
    kind: ModuleDependencyKind,
    mode: ModuleDependencyMode,
    specifier: string | null,
    node: ts.Node,
  ): void => {
    if (specifier !== null || mode === 'runtime') {
      dependencies.push({ kind, mode, specifier, location: location(node) });
    }
  };

  const visit = (node: ts.Node): void => {
    if (ts.isImportDeclaration(node)) {
      add(
        node.importClause ? 'static-import' : 'side-effect-import',
        importMode(node),
        literalText(node.moduleSpecifier),
        node,
      );
      return;
    }
    if (ts.isExportDeclaration(node)) {
      if (node.moduleSpecifier) {
        add('re-export', exportMode(node), literalText(node.moduleSpecifier), node);
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
      );
      return;
    }
    if (ts.isExportAssignment(node)) {
      const dependency = moduleCallSpecifier(node.expression);
      if (dependency) {
        add('export-assignment', 'runtime', dependency.specifier, node);
        return;
      }
    }
    if (ts.isCallExpression(node)) {
      const dependency = moduleCallSpecifier(node);
      if (dependency) {
        add(dependency.kind, 'runtime', dependency.specifier, node);
        return;
      }
    }
    ts.forEachChild(node, visit);
  };

  visit(sourceFile);
  return dependencies;
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
