import type { ResolvedModuleDependency } from '@/tests/helpers/moduleDependencyAudit';
import type { ResolvedStylesheetDependency } from '@/tests/helpers/stylesheetDependencyAudit';
import { FRONTEND_LAYERS } from './frontendArchitecturePolicy';
import type {
  AssetDependencyPolicy,
  FrontendAssetAuditContext,
  FrontendAssetDependencyError,
  FrontendAssetDependencyReport,
  FrontendFinding,
  FrontendLayer,
  RepositoryAssetDependencyAllowance,
} from './frontendArchitectureModel';

export const FRONTEND_ASSET_DEPENDENCY_POLICY: AssetDependencyPolicy = {
  uses: [
    {
      sourceLayer: 'app-composition',
      mode: 'runtime',
      dependencyKind: 'side-effect-import',
      resourceKind: 'stylesheet',
      consumerSourceFile: 'src/app/App.tsx',
      repositoryRelativeAssetPath: 'src/app/App.css',
    },
    {
      sourceLayer: 'app-composition',
      mode: 'runtime',
      dependencyKind: 'side-effect-import',
      resourceKind: 'stylesheet',
      consumerSourceFile: 'src/app/main.tsx',
      repositoryRelativeAssetPath: 'src/app/workbench-dockview.css',
    },
  ],
};

type RepositoryAssetDependency = ResolvedModuleDependency | ResolvedStylesheetDependency;

function isCanonicalSourcePath(path: string): boolean {
  return path.startsWith('src/')
    && !path.includes('\\')
    && !path.split('/').some((segment) => segment === '' || segment === '.' || segment === '..');
}

function rowKey(row: RepositoryAssetDependencyAllowance): string {
  return [
    row.sourceLayer,
    row.mode,
    row.dependencyKind,
    row.resourceKind,
    row.consumerSourceFile,
    row.repositoryRelativeAssetPath,
  ].join('\u0000');
}

function validateAssetPolicy(
  context: FrontendAssetAuditContext,
  policy: AssetDependencyPolicy,
): FrontendAssetDependencyError[] {
  const errors: FrontendAssetDependencyError[] = [];
  const seen = new Set<string>();
  const productionConsumers = new Set(
    context.productionSources.map(({ path }) => path.replace(/\\/g, '/')),
  );
  const stylesheetConsumers = new Set(context.stylesheetGraph.repositoryStylesheets);
  const consumers = new Set([...productionConsumers, ...stylesheetConsumers]);
  const stylesheets = new Set(context.stylesheetGraph.repositoryStylesheets);
  for (const row of policy.uses) {
    const key = rowKey(row);
    let reason: string | null = null;
    if (seen.has(key)) reason = 'duplicate-row';
    else if (!FRONTEND_LAYERS.includes(row.sourceLayer)) reason = 'unknown-source-layer';
    else if (row.mode !== 'runtime' && row.mode !== 'build-style') reason = 'unsupported-mode';
    else if (!isCanonicalSourcePath(row.consumerSourceFile)) reason = 'noncanonical-consumer';
    else if (!isCanonicalSourcePath(row.repositoryRelativeAssetPath)
      || !row.repositoryRelativeAssetPath.endsWith('.css')) reason = 'noncanonical-asset-path';
    else if (!consumers.has(row.consumerSourceFile)) reason = 'consumer-not-in-production-graph';
    else if (!stylesheets.has(row.repositoryRelativeAssetPath)) reason = 'asset-target-missing';
    else if (row.resourceKind !== 'stylesheet') reason = 'unsupported-resource-kind';
    else if (row.mode === 'runtime' && !productionConsumers.has(row.consumerSourceFile)) {
      reason = 'runtime-asset-consumer-not-typescript';
    }
    else if (row.mode === 'build-style' && !stylesheetConsumers.has(row.consumerSourceFile)) {
      reason = 'build-style-consumer-not-stylesheet';
    }
    else if (row.mode === 'runtime' && row.dependencyKind !== 'side-effect-import') {
      reason = 'runtime-asset-must-be-side-effect-import';
    } else if (row.mode === 'build-style'
      && row.dependencyKind !== 'stylesheet-import'
      && row.dependencyKind !== 'stylesheet-url') {
      reason = 'build-style-asset-must-be-stylesheet-reference';
    }
    if (reason !== null) {
      errors.push({
        kind: 'invalid-asset-policy-row',
        consumerSourceFile: row.consumerSourceFile,
        repositoryRelativeAssetPath: row.repositoryRelativeAssetPath,
        reason,
      });
    }
    seen.add(key);
  }
  return errors;
}

function assetDependencies(context: FrontendAssetAuditContext): RepositoryAssetDependency[] {
  return [...context.moduleDependencies, ...context.stylesheetGraph.dependencies]
    .filter((dependency) => dependency.origin.kind === 'repository-asset')
    .sort((left, right) => [
      left.repositoryRelativeSourceFile,
      left.canonicalOriginTarget,
      left.kind,
    ].join('\u0000').localeCompare([
      right.repositoryRelativeSourceFile,
      right.canonicalOriginTarget,
      right.kind,
    ].join('\u0000')));
}

function matchesRow(
  row: RepositoryAssetDependencyAllowance,
  dependency: RepositoryAssetDependency,
  sourceLayer: FrontendLayer,
): boolean {
  if (dependency.origin.kind !== 'repository-asset') return false;
  return row.sourceLayer === sourceLayer
    && row.mode === dependency.mode
    && row.dependencyKind === dependency.kind
    && row.resourceKind === dependency.origin.asset.resourceKind
    && row.consumerSourceFile === dependency.repositoryRelativeSourceFile
    && row.repositoryRelativeAssetPath === dependency.origin.asset.repositoryRelativeAssetPath;
}

function finding(
  dependency: RepositoryAssetDependency,
  sourceLayer: FrontendLayer,
): FrontendFinding {
  const location = 'location' in dependency
    ? dependency.location
    : { line: dependency.line, column: dependency.column };
  return {
    ruleId: 'frontend.asset.consumer-path',
    repositoryRelativeSourceFile: dependency.repositoryRelativeSourceFile,
    fullyQualifiedOwner: dependency.fullyQualifiedOwner,
    dependencyKind: dependency.kind,
    canonicalOriginTarget: dependency.canonicalOriginTarget,
    sourceLayer,
    targetLayer: null,
    importedSymbol: null,
    ...location,
  };
}

export function auditFrontendAssetDependencies(
  context: FrontendAssetAuditContext,
  classification: ReadonlyMap<string, FrontendLayer>,
  policy: AssetDependencyPolicy,
): FrontendAssetDependencyReport {
  const errors: FrontendAssetDependencyError[] = [
    ...context.stylesheetGraph.errors,
    ...validateAssetPolicy(context, policy),
  ];
  const findings: FrontendFinding[] = [];
  const inherited = new Map<string, Set<FrontendLayer>>();
  const dependencies = assetDependencies(context);

  const inherit = (target: string, layer: FrontendLayer): boolean => {
    let layers = inherited.get(target);
    if (!layers) {
      layers = new Set();
      inherited.set(target, layers);
    }
    const previousSize = layers.size;
    layers.add(layer);
    return layers.size !== previousSize;
  };

  const audited = new Set<RepositoryAssetDependency>();
  let changed = true;
  while (changed) {
    changed = false;
    for (const dependency of dependencies) {
      const sourceLayer = 'location' in dependency
        ? classification.get(dependency.repositoryRelativeSourceFile)
        : (() => {
            const layers = inherited.get(dependency.repositoryRelativeSourceFile);
            return layers?.size === 1 ? [...layers][0] : undefined;
          })();
      if (!sourceLayer) continue;
      const allowed = policy.uses.some((row) => matchesRow(row, dependency, sourceLayer));
      if (!audited.has(dependency)) {
        if (!allowed) findings.push(finding(dependency, sourceLayer));
        audited.add(dependency);
      }
      if (allowed && dependency.origin.kind === 'repository-asset') {
        changed = inherit(dependency.origin.asset.repositoryRelativeAssetPath, sourceLayer) || changed;
      }
    }
  }

  const stylesheetLayers = new Map<string, FrontendLayer>();
  for (const [sourceFile, layers] of [...inherited].sort(([left], [right]) => left.localeCompare(right))) {
    const orderedLayers = FRONTEND_LAYERS.filter((layer) => layers.has(layer));
    if (orderedLayers.length === 1) stylesheetLayers.set(sourceFile, orderedLayers[0]);
    else if (orderedLayers.length > 1) {
      errors.push({ kind: 'stylesheet-layer-conflict', sourceFile, inheritedLayers: orderedLayers });
    }
  }

  return { findings, errors, stylesheetLayers };
}
