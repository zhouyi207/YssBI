import * as ts from 'typescript/unstable/ast';
import type { Symbol as TypeScriptSymbol } from 'typescript/unstable/sync';

import type {
  ArchitectureSource,
  ResolvedModuleDependency,
} from '@/tests/helpers/moduleDependencyAudit';
import { resolvedModuleDependencies } from '@/tests/helpers/moduleDependencyAudit';
import { rawTauriInvokeOccurrences } from '@/tests/helpers/tauriInvokeAudit';
import type { TypeScriptAuditProject } from '@/tests/helpers/typescriptAudit';
import { classifyFrontendSources } from './frontendArchitecturePolicy';
import type {
  FrontendArchitecturePolicy,
  FrontendFinding,
  FrontendLayer,
  FrontendResolvedCapability,
} from './frontendArchitectureModel';

const RAW_INVOKE_ADAPTER = 'src/services/ipc/invokeCommand.ts';
const RAW_DIALOG_ADAPTER = 'src/services/platform/pathDialog.ts';
const ROOT_DOCKVIEW_CONSUMER = 'src/views/EditorView/Layout/Workspace.tsx';
const NESTED_DOCKVIEW_CONSUMER = 'src/views/LogView/LogWorkspaceDockview.tsx';

const PUBLICATION_MEMBERS = new Set([
  'applyProjection',
  'hydrateProjection',
  'publish',
  'replaceProjection',
  'setState',
  'submitPublication',
]);

function findingSort(left: FrontendFinding, right: FrontendFinding): number {
  return [
    left.ruleId,
    left.repositoryRelativeSourceFile,
    left.fullyQualifiedOwner,
    left.dependencyKind,
    left.canonicalOriginTarget,
    left.line.toString().padStart(8, '0'),
    left.column.toString().padStart(8, '0'),
  ].join('\u0000').localeCompare([
    right.ruleId,
    right.repositoryRelativeSourceFile,
    right.fullyQualifiedOwner,
    right.dependencyKind,
    right.canonicalOriginTarget,
    right.line.toString().padStart(8, '0'),
    right.column.toString().padStart(8, '0'),
  ].join('\u0000'));
}

function locationOf(sourceFile: ts.SourceFile, node: ts.Node): { line: number; column: number } {
  const position = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return { line: position.line + 1, column: position.character + 1 };
}

function semanticFinding(
  ruleId: string,
  sourceFile: string,
  sourceLayer: FrontendLayer,
  dependencyKind: FrontendFinding['dependencyKind'],
  canonicalOriginTarget: string,
  targetLayer: FrontendLayer | null,
  importedSymbol: string | null,
  location: { line: number; column: number },
): FrontendFinding {
  return {
    ruleId,
    repositoryRelativeSourceFile: sourceFile,
    fullyQualifiedOwner: `${sourceFile}::<module>`,
    dependencyKind,
    canonicalOriginTarget,
    sourceLayer,
    targetLayer,
    importedSymbol,
    line: location.line,
    column: location.column,
  };
}

function exactCapability(
  policy: FrontendArchitecturePolicy,
  dependency: ResolvedModuleDependency,
  sourceLayer: FrontendLayer,
): FrontendResolvedCapability | null {
  if (dependency.origin.kind !== 'repository-module' || dependency.importedSymbol === null) {
    return null;
  }
  const canonicalModule = dependency.origin.declarationTarget;
  return policy.capabilities.find((capability) => (
    capability.sourceLayer === sourceLayer
    && capability.canonicalModule === canonicalModule
    && capability.exportedSymbols.includes(dependency.importedSymbol!)
    && (capability.exactConsumers === null
      || capability.exactConsumers.includes(dependency.repositoryRelativeSourceFile))
  )) ?? null;
}

function dependencyAt(
  sourceFile: ts.SourceFile,
  dependencies: readonly ResolvedModuleDependency[],
  node: ts.Node,
  importedSymbol: string | null | undefined,
): ResolvedModuleDependency | null {
  const location = locationOf(sourceFile, node);
  return dependencies.find((dependency) => (
    dependency.location.line === location.line
    && dependency.location.column === location.column
    && (importedSymbol === undefined
      || importedSymbol === null
      || dependency.importedSymbol === importedSymbol)
  )) ?? dependencies.find((dependency) => (
    dependency.location.line === location.line
    && dependency.location.column === location.column
  )) ?? null;
}

function importedBindings(
  context: TypeScriptAuditProject,
  sourceFile: ts.SourceFile,
  dependencies: readonly ResolvedModuleDependency[],
): ReadonlyMap<TypeScriptSymbol, ResolvedModuleDependency> {
  const bindings = new Map<TypeScriptSymbol, ResolvedModuleDependency>();
  for (const statement of sourceFile.statements) {
    if (!ts.isImportDeclaration(statement) || !statement.importClause) continue;
    const { importClause } = statement;
    if (importClause.name) {
      const symbol = context.checker.getSymbolAtLocation(importClause.name);
      const dependency = dependencyAt(sourceFile, dependencies, statement, undefined);
      if (symbol && dependency) bindings.set(symbol, dependency);
    }
    const namedBindings = importClause.namedBindings;
    if (!namedBindings) continue;
    if (ts.isNamespaceImport(namedBindings)) {
      const symbol = context.checker.getSymbolAtLocation(namedBindings.name);
      const dependency = dependencyAt(sourceFile, dependencies, statement, null);
      if (symbol && dependency) bindings.set(symbol, dependency);
      continue;
    }
    for (const element of namedBindings.elements) {
      const symbol = context.checker.getSymbolAtLocation(element.name);
      const dependency = dependencyAt(
        sourceFile,
        dependencies,
        element,
        (element.propertyName ?? element.name).text,
      );
      if (symbol && dependency) bindings.set(symbol, dependency);
    }
  }
  return bindings;
}

function importedDependencyForExpression(
  expression: ts.Expression,
  context: TypeScriptAuditProject,
  bindings: ReadonlyMap<TypeScriptSymbol, ResolvedModuleDependency>,
): ResolvedModuleDependency | null {
  if (ts.isIdentifier(expression)) {
    const symbol = context.checker.getSymbolAtLocation(expression);
    return symbol ? bindings.get(symbol) ?? null : null;
  }
  if (ts.isPropertyAccessExpression(expression)) {
    return importedDependencyForExpression(expression.expression, context, bindings);
  }
  if (ts.isCallExpression(expression)) {
    return importedDependencyForExpression(expression.expression, context, bindings);
  }
  if (ts.isParenthesizedExpression(expression)) {
    return importedDependencyForExpression(expression.expression, context, bindings);
  }
  return null;
}

function memberCapabilityAllows(
  capability: FrontendResolvedCapability,
  member: string,
): boolean {
  if (capability.memberCapabilities === null) return true;
  return Object.values(capability.memberCapabilities).some((members) => members.includes(member));
}

function auditResolvedImports(
  source: ArchitectureSource,
  sourceLayer: FrontendLayer,
  classification: ReadonlyMap<string, FrontendLayer>,
  dependencies: readonly ResolvedModuleDependency[],
  policy: FrontendArchitecturePolicy,
): FrontendFinding[] {
  const findings: FrontendFinding[] = [];
  for (const dependency of dependencies) {
    if (dependency.origin.kind === 'external'
      && dependency.origin.dependency.packageName === '@tauri-apps/plugin-dialog'
      && source.path !== RAW_DIALOG_ADAPTER) {
      findings.push(semanticFinding(
        'frontend.dialog.raw',
        source.path,
        sourceLayer,
        dependency.kind,
        dependency.canonicalOriginTarget,
        null,
        dependency.importedSymbol,
        dependency.location,
      ));
      continue;
    }
    if (dependency.origin.kind !== 'repository-module') continue;
    const targetLayer = classification.get(dependency.origin.declarationTarget) ?? null;
    const capability = exactCapability(policy, dependency, sourceLayer);
    if (sourceLayer === 'views' && targetLayer === 'core' && capability === null) {
      findings.push(semanticFinding(
        'frontend.view-core.capability',
        source.path,
        sourceLayer,
        dependency.kind,
        dependency.canonicalOriginTarget,
        targetLayer,
        dependency.importedSymbol,
        dependency.location,
      ));
    }
    const rawServiceTransport = targetLayer === 'services'
      && dependency.origin.declarationTarget.startsWith('src/services/ipc/');
    const rawWireSymbol = targetLayer === 'wire-schema' && capability === null;
    if (sourceLayer === 'application' && (rawServiceTransport || rawWireSymbol)) {
      findings.push(semanticFinding(
        'frontend.application.raw-wire',
        source.path,
        sourceLayer,
        dependency.kind,
        dependency.canonicalOriginTarget,
        targetLayer,
        dependency.importedSymbol,
        dependency.location,
      ));
    }
  }
  return findings;
}

function dockviewRule(sourceFile: string): {
  ruleId: 'frontend.dockview.root-constructor' | 'frontend.dockview.nested-constructor';
  allowedPath: string;
} {
  return sourceFile.startsWith('src/views/LogView/')
    ? { ruleId: 'frontend.dockview.nested-constructor', allowedPath: NESTED_DOCKVIEW_CONSUMER }
    : { ruleId: 'frontend.dockview.root-constructor', allowedPath: ROOT_DOCKVIEW_CONSUMER };
}

function auditSourceExpressions(
  context: TypeScriptAuditProject,
  source: ArchitectureSource,
  sourceLayer: FrontendLayer,
  classification: ReadonlyMap<string, FrontendLayer>,
  dependencies: readonly ResolvedModuleDependency[],
  policy: FrontendArchitecturePolicy,
): FrontendFinding[] {
  const sourceFile = context.sourceFile(source.path);
  const bindings = importedBindings(context, sourceFile, dependencies);
  const findings: FrontendFinding[] = [];
  const visit = (node: ts.Node): void => {
    if (ts.isPropertyAccessExpression(node)) {
      const dependency = importedDependencyForExpression(node.expression, context, bindings);
      if (dependency?.origin.kind === 'repository-module') {
        const targetLayer = classification.get(dependency.origin.declarationTarget) ?? null;
        const capability = exactCapability(policy, dependency, sourceLayer);
        if (capability && !memberCapabilityAllows(capability, node.name.text)) {
          findings.push(semanticFinding(
            'frontend.projection-read-mutation',
            source.path,
            sourceLayer,
            'property-access',
            dependency.canonicalOriginTarget,
            targetLayer,
            dependency.importedSymbol,
            locationOf(sourceFile, node),
          ));
        }
        if (sourceLayer === 'views'
          && targetLayer === 'core'
          && capability === null
          && PUBLICATION_MEMBERS.has(node.name.text)) {
          findings.push(semanticFinding(
            'frontend.view-publication',
            source.path,
            sourceLayer,
            'property-access',
            dependency.canonicalOriginTarget,
            targetLayer,
            dependency.importedSymbol,
            locationOf(sourceFile, node),
          ));
        }
        if (sourceLayer === 'services'
          && targetLayer === 'core'
          && node.name.text === 'setState'
          && ts.isCallExpression(node.parent)
          && node.parent.expression === node) {
          findings.push(semanticFinding(
            'frontend.service-projection-write',
            source.path,
            sourceLayer,
            'call',
            dependency.canonicalOriginTarget,
            targetLayer,
            dependency.importedSymbol,
            locationOf(sourceFile, node.parent),
          ));
        }
      }
    }
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const dependency = ts.isIdentifier(node.tagName)
        ? importedDependencyForExpression(node.tagName, context, bindings)
        : null;
      if (dependency?.origin.kind === 'external'
        && dependency.origin.dependency.packageName === 'dockview-react'
        && dependency.importedSymbol === 'DockviewReact') {
        const rule = dockviewRule(source.path);
        if (source.path !== rule.allowedPath) {
          findings.push(semanticFinding(
            rule.ruleId,
            source.path,
            sourceLayer,
            'constructor',
            dependency.canonicalOriginTarget,
            null,
            dependency.importedSymbol,
            locationOf(sourceFile, node),
          ));
        }
      }
    }
    node.forEachChild(visit);
  };
  visit(sourceFile);
  return findings;
}

export function auditFrontendSemantics(
  context: TypeScriptAuditProject,
  sources: readonly ArchitectureSource[],
  policy: FrontendArchitecturePolicy,
): readonly FrontendFinding[] {
  const sourcePaths = new Set(sources.map(({ path }) => path));
  const classification = classifyFrontendSources(sources).classification;
  const findings: FrontendFinding[] = rawTauriInvokeOccurrences(context)
    .filter((occurrence) => sourcePaths.has(occurrence.repositoryRelativeSourceFile)
      && occurrence.repositoryRelativeSourceFile !== RAW_INVOKE_ADAPTER)
    .map((occurrence) => semanticFinding(
      'frontend.invoke.raw',
      occurrence.repositoryRelativeSourceFile,
      classification.get(occurrence.repositoryRelativeSourceFile)!,
      'call',
      'external:@tauri-apps/api::core',
      null,
      'invoke',
      occurrence,
    ));

  for (const source of sources) {
    const sourceLayer = classification.get(source.path);
    if (!sourceLayer || !['application', 'services', 'views'].includes(sourceLayer)) continue;
    const dependencies = resolvedModuleDependencies(context, source);
    findings.push(...auditResolvedImports(
      source,
      sourceLayer,
      classification,
      dependencies,
      policy,
    ));
    findings.push(...auditSourceExpressions(
      context,
      source,
      sourceLayer,
      classification,
      dependencies,
      policy,
    ));
  }
  return findings.sort(findingSort);
}
