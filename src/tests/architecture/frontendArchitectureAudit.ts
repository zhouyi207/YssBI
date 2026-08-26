import {
  ModuleDependencyResolutionError,
  type ResolvedModuleDependency,
} from '@/tests/helpers/moduleDependencyAudit';
import { type TypeScriptAuditProject } from '@/tests/helpers/typescriptAudit';
import {
  auditFrontendAssetDependencies,
} from './frontendAssetDependencyPolicy';
import {
  FRONTEND_ARCHITECTURE_DEBT,
  compareExactFrontendDebt,
} from './frontendArchitectureDebt';
import {
  classifyFrontendSources,
} from './frontendArchitecturePolicy';
import {
  auditFrontendExternalDependencies,
} from './frontendExternalDependencyPolicy';
import {
  productionTypeScriptSources,
  resolvedModuleDependencies,
  resolvedStylesheetDependencies,
  type AssetDependencyPolicy,
  type ExternalDependencyPolicy,
  type FrontendArchitecturePolicy,
  type FrontendArchitectureReport,
  type FrontendFinding,
  type FrontendLayer,
  type ReadonlyPackageManifest,
  type RepositoryTextReader,
} from './frontendArchitectureModel';

function hasLayerEdge(
  policy: FrontendArchitecturePolicy,
  sourceLayer: FrontendLayer,
  targetLayer: FrontendLayer,
): boolean {
  return policy.layerEdges.some(([source, target]) => (
    source === sourceLayer && target === targetLayer
  ));
}

function hasExactCapability(
  policy: FrontendArchitecturePolicy,
  dependency: ResolvedModuleDependency,
  sourceLayer: FrontendLayer,
): boolean {
  if (dependency.origin.kind !== 'repository-module' || dependency.importedSymbol === null) return false;
  const targetModule = dependency.origin.declarationTarget;
  const importedSymbol = dependency.importedSymbol;
  return policy.capabilities.some((capability) => (
    capability.sourceLayer === sourceLayer
    && capability.canonicalModule === targetModule
    && capability.exportedSymbols.includes(importedSymbol)
    && (capability.exactConsumers === null
      || capability.exactConsumers.includes(dependency.repositoryRelativeSourceFile))
  ));
}

function internalFinding(
  dependency: ResolvedModuleDependency,
  sourceLayer: FrontendLayer,
  targetLayer: FrontendLayer,
  policy: FrontendArchitecturePolicy,
): FrontendFinding {
  const capabilityGoverned = policy.capabilities.some((capability) => (
    capability.sourceLayer === sourceLayer
    && dependency.origin.kind === 'repository-module'
    && capability.canonicalModule === dependency.origin.declarationTarget
  ));
  return {
    ruleId: capabilityGoverned
      ? 'frontend.capability.exact-origin-symbol-consumer'
      : `frontend.layer.${sourceLayer}-to-${targetLayer}`,
    repositoryRelativeSourceFile: dependency.repositoryRelativeSourceFile,
    fullyQualifiedOwner: dependency.fullyQualifiedOwner,
    dependencyKind: dependency.kind,
    canonicalOriginTarget: dependency.canonicalOriginTarget,
    sourceLayer,
    targetLayer,
    importedSymbol: dependency.importedSymbol,
    line: dependency.location.line,
    column: dependency.location.column,
  };
}

function auditInternalDependencies(
  dependencies: readonly ResolvedModuleDependency[],
  classification: ReadonlyMap<string, FrontendLayer>,
  policy: FrontendArchitecturePolicy,
): FrontendFinding[] {
  const findings: FrontendFinding[] = [];
  for (const dependency of dependencies) {
    if (dependency.origin.kind !== 'repository-module') continue;
    const sourceLayer = classification.get(dependency.repositoryRelativeSourceFile);
    const targetLayer = classification.get(dependency.origin.declarationTarget);
    if (!sourceLayer || !targetLayer || sourceLayer === targetLayer) continue;
    if (hasLayerEdge(policy, sourceLayer, targetLayer)) continue;
    if (hasExactCapability(policy, dependency, sourceLayer)) continue;
    findings.push(internalFinding(dependency, sourceLayer, targetLayer, policy));
  }
  return findings;
}

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

export function auditFrontendArchitectureDependencies(
  context: TypeScriptAuditProject,
  repositoryRoot: string,
  sourceReader: RepositoryTextReader,
  policy: FrontendArchitecturePolicy,
  externalPolicy: ExternalDependencyPolicy,
  assetPolicy: AssetDependencyPolicy,
  packageJson: ReadonlyPackageManifest,
): FrontendArchitectureReport {
  const productionSources = productionTypeScriptSources(context);
  const discoveredClassification = classifyFrontendSources(productionSources);
  const unresolvedErrors: ModuleDependencyResolutionError[] = [];
  const moduleDependencies: ResolvedModuleDependency[] = [];
  for (const source of productionSources) {
    try {
      moduleDependencies.push(...resolvedModuleDependencies(context, source));
    } catch (error) {
      if (!(error instanceof ModuleDependencyResolutionError)) throw error;
      unresolvedErrors.push(error);
    }
  }
  const stylesheetGraph = resolvedStylesheetDependencies(
    repositoryRoot,
    moduleDependencies,
    sourceReader,
  );
  const missingTargets = [...new Set(moduleDependencies.flatMap((dependency) => (
    dependency.origin.kind === 'repository-module'
      && !discoveredClassification.classification.has(dependency.origin.declarationTarget)
      ? [dependency.origin.declarationTarget]
      : []
  )))].sort();
  const classification = {
    classification: discoveredClassification.classification,
    errors: [
      ...discoveredClassification.errors,
      ...missingTargets.map((sourceFile) => ({
        kind: 'unclassified-production-source' as const,
        sourceFile,
      })),
    ],
  };
  const asset = auditFrontendAssetDependencies({
    productionSources,
    moduleDependencies,
    stylesheetGraph,
  }, classification.classification, assetPolicy);
  const external = auditFrontendExternalDependencies(
    [...moduleDependencies, ...stylesheetGraph.dependencies],
    classification.classification,
    asset.stylesheetLayers,
    packageJson,
    externalPolicy,
  );
  const findings = [
    ...auditInternalDependencies(moduleDependencies, classification.classification, policy),
    ...external.findings,
    ...asset.findings,
  ].sort(findingSort);
  return {
    classification,
    unresolvedErrors: unresolvedErrors.sort((left, right) => (
      `${left.sourceFile}:${left.line}:${left.column}`.localeCompare(
        `${right.sourceFile}:${right.line}:${right.column}`,
      )
    )),
    moduleDependencies,
    stylesheetGraph,
    external,
    asset,
    findings,
    debt: compareExactFrontendDebt(findings, FRONTEND_ARCHITECTURE_DEBT),
  };
}
