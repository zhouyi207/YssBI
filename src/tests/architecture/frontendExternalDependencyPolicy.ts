import type {
  ExternalDependencyAllowance,
  ExternalDependencyPolicy,
  FrontendDependencyMode,
  FrontendExternalDependencyError,
  FrontendExternalDependencyEvaluation,
  FrontendExternalDependencyReport,
  FrontendFinding,
  FrontendLayer,
  ReadonlyPackageManifest,
  ResolvedFrontendDependency,
} from './frontendArchitectureModel';
import { FRONTEND_LAYERS } from './frontendArchitecturePolicy';

const DECLARED_RUNTIME_PACKAGES = [
  '@dnd-kit/core',
  '@fontsource-variable/inter',
  '@tanstack/react-virtual',
  '@tauri-apps/api',
  '@tauri-apps/plugin-clipboard-manager',
  '@tauri-apps/plugin-dialog',
  '@tauri-apps/plugin-opener',
  'ag-grid-community',
  'ag-grid-react',
  'class-variance-authority',
  'clsx',
  'd3',
  'dockview-react',
  'i18next',
  'katex',
  'lodash',
  'marked',
  'pinyin-pro',
  'radix-ui',
  'react',
  'react-dom',
  'react-i18next',
  'react-icons',
  'react-markdown',
  'react-responsive-carousel',
  'react-router',
  'rehype-katex',
  'remark-math',
  'shadcn',
  'tailwind-merge',
  'tw-animate-css',
  'zustand',
] as const;

function allowance(
  sourceLayer: FrontendLayer,
  mode: FrontendDependencyMode,
  packageName: string,
  canonicalSubpaths: readonly (string | null)[],
  resourceKind: 'module' | 'stylesheet' = 'module',
  consumerSourceFile: string | null = null,
): ExternalDependencyAllowance {
  return {
    sourceLayer,
    mode,
    packageName,
    resourceKind,
    canonicalSubpaths,
    consumerSourceFile,
  };
}

function pairedModuleAllowances(
  sourceLayer: FrontendLayer,
  packages: readonly (readonly [string, readonly (string | null)[]])[],
): ExternalDependencyAllowance[] {
  return packages.flatMap(([packageName, subpaths]) => [
    allowance(sourceLayer, 'runtime', packageName, subpaths),
    allowance(sourceLayer, 'type-only', packageName, subpaths),
  ]);
}

const APP_USES = [
  ...pairedModuleAllowances('app-composition', [
    ['react', [null]],
    ['react-router', [null]],
    ['i18next', [null]],
    ['react-i18next', [null]],
  ]),
  allowance('app-composition', 'runtime', 'react-dom', ['client']),
  allowance(
    'app-composition',
    'runtime',
    'dockview-react',
    ['dist::styles::dockview.css'],
    'stylesheet',
  ),
  allowance('app-composition', 'build-style', 'tailwindcss', [null], 'stylesheet', 'src/app/App.css'),
  allowance('app-composition', 'build-style', 'tw-animate-css', [null], 'stylesheet', 'src/app/App.css'),
  allowance('app-composition', 'build-style', 'shadcn', ['tailwind.css'], 'stylesheet', 'src/app/App.css'),
  allowance(
    'app-composition',
    'build-style',
    '@fontsource-variable/inter',
    [null],
    'stylesheet',
    'src/app/App.css',
  ),
];

const VIEW_USES = [
  ...pairedModuleAllowances('views', [
    ['react', [null]],
    ['react-dom', [null]],
    ['react-router', [null]],
    ['i18next', [null]],
    ['react-i18next', [null]],
    ['@dnd-kit/core', [null]],
    ['@tanstack/react-virtual', [null]],
    ['ag-grid-community', [null]],
    ['ag-grid-react', [null]],
    ['d3', [null]],
    ['dockview-react', [null]],
    ['zustand', ['react::shallow']],
  ]),
  allowance('views', 'runtime', 'katex', [null]),
  allowance('views', 'runtime', 'react-icons', [null, 'fi', 'vsc']),
  allowance('views', 'runtime', 'katex', ['dist::katex.min.css'], 'stylesheet'),
];

const APPLICATION_USES = [
  ...pairedModuleAllowances('application', [
    ['react', [null]],
    ['react-router', [null]],
    ['i18next', [null]],
    ['react-i18next', [null]],
    ['zustand', ['react::shallow']],
  ]),
  allowance('application', 'runtime', '@dnd-kit/core', [null]),
  allowance('application', 'type-only', 'dockview-react', [null]),
];

const CORE_USES = [
  ...pairedModuleAllowances('core', [
    ['react', [null]],
    ['i18next', [null]],
    ['react-i18next', [null]],
    ['zustand', [null, 'react::shallow', 'vanilla']],
    ['@dnd-kit/core', [null]],
    ['ag-grid-community', [null]],
    ['ag-grid-react', [null]],
  ]),
  allowance('core', 'type-only', 'dockview-react', [null]),
];

const COMPONENT_USES = [
  ...pairedModuleAllowances('components-ui', [
    ['react', [null]],
    ['radix-ui', [null]],
    ['class-variance-authority', [null]],
    ['clsx', [null]],
    ['tailwind-merge', [null]],
    ['d3', [null]],
    ['dockview-react', [null]],
    ['ag-grid-community', [null]],
  ]),
  allowance('components-ui', 'runtime', 'react-markdown', [null]),
  allowance('components-ui', 'runtime', 'rehype-katex', [null]),
  allowance('components-ui', 'runtime', 'remark-math', [null]),
  allowance('components-ui', 'runtime', 'react-icons', [null, 'bs', 'fi', 'vsc']),
  allowance('components-ui', 'runtime', 'katex', ['dist::katex.min.css'], 'stylesheet'),
];

export const FRONTEND_EXTERNAL_DEPENDENCY_POLICY: ExternalDependencyPolicy = {
  declaredRuntimePackages: DECLARED_RUNTIME_PACKAGES,
  declaredBuildOnlyPackages: ['tailwindcss'],
  uses: [
    ...APP_USES,
    ...VIEW_USES,
    ...APPLICATION_USES,
    ...CORE_USES,
    allowance('domain', 'runtime', 'pinyin-pro', [null]),
    ...COMPONENT_USES,
    allowance('pure-shared', 'runtime', 'clsx', [null]),
    allowance('pure-shared', 'runtime', 'tailwind-merge', [null]),
    allowance('services', 'runtime', '@tauri-apps/api', ['core', 'event', 'webviewWindow', 'window']),
    allowance('services', 'runtime', '@tauri-apps/plugin-clipboard-manager', [null]),
    allowance('services', 'runtime', '@tauri-apps/plugin-dialog', [null]),
    allowance('services', 'runtime', '@tauri-apps/plugin-opener', [null]),
  ],
};

function duplicates(values: readonly string[]): string[] {
  const seen = new Set<string>();
  return [...new Set(values.filter((value) => {
    if (seen.has(value)) return true;
    seen.add(value);
    return false;
  }))].sort();
}

function setDifference(left: ReadonlySet<string>, right: ReadonlySet<string>): string[] {
  return [...left].filter((value) => !right.has(value)).sort();
}

function declaresPackage(
  declarations: Readonly<Record<string, string>>,
  packageName: string,
): boolean {
  return Object.prototype.hasOwnProperty.call(declarations, packageName);
}

function canonicalPackageName(packageName: string): boolean {
  const lower = packageName.toLowerCase();
  if (!packageName
    || packageName.includes('*')
    || packageName.includes('\\')
    || lower.includes('%2f')
    || lower.includes('%5c')) return false;
  const segments = packageName.split('/');
  if (segments.some((segment) => !segment || segment === '.' || segment === '..')) return false;
  return packageName.startsWith('@') ? segments.length === 2 : segments.length === 1;
}

function canonicalConsumerPath(path: string): boolean {
  return path.startsWith('src/')
    && !path.includes('\\')
    && !path.split('/').some((segment) => segment === '' || segment === '.' || segment === '..');
}

function canonicalSubpath(subpath: string | null): boolean {
  if (subpath === null) return true;
  return subpath.length > 0
    && !subpath.includes('/')
    && !subpath.includes('\\')
    && !subpath.includes('*')
    && !subpath.toLowerCase().includes('%2f')
    && !subpath.toLowerCase().includes('%5c')
    && subpath.split('::').every((segment) => segment && segment !== '.' && segment !== '..');
}

function allowanceKey(row: ExternalDependencyAllowance): string {
  return [
    row.sourceLayer,
    row.mode,
    row.packageName,
    row.resourceKind,
    [...row.canonicalSubpaths].sort((left, right) => String(left).localeCompare(String(right))).join(','),
    row.consumerSourceFile ?? '',
  ].join('\u0000');
}

function validatePolicy(
  packageJson: ReadonlyPackageManifest,
  policy: ExternalDependencyPolicy,
  stylesheetLayers: ReadonlyMap<string, FrontendLayer>,
): FrontendExternalDependencyError[] {
  const errors: FrontendExternalDependencyError[] = [];
  const expectedRuntime = new Set(DECLARED_RUNTIME_PACKAGES);
  const declaredRuntime = new Set(policy.declaredRuntimePackages);
  const actualRuntime = new Set(Object.keys(packageJson.dependencies));
  const duplicateRuntime = duplicates(policy.declaredRuntimePackages);
  for (const packageName of duplicateRuntime) {
    errors.push({ kind: 'invalid-external-policy-row', packageName, reason: 'duplicate-runtime-declaration' });
  }
  const invalidDeclaredRuntime = [
    ...setDifference(expectedRuntime, declaredRuntime).map((packageName) => `missing:${packageName}`),
    ...setDifference(declaredRuntime, expectedRuntime).map((packageName) => `extra:${packageName}`),
  ];
  for (const reason of invalidDeclaredRuntime) {
    errors.push({ kind: 'invalid-external-policy-row', packageName: reason.split(':').slice(1).join(':'), reason });
  }
  const missing = setDifference(declaredRuntime, actualRuntime);
  const extra = setDifference(actualRuntime, declaredRuntime);
  if (missing.length > 0 || extra.length > 0) {
    errors.push({ kind: 'production-declaration-set-mismatch', missing, extra });
  }

  const duplicateBuild = duplicates(policy.declaredBuildOnlyPackages);
  const expectedBuild = new Set(['tailwindcss']);
  const declaredBuild = new Set(policy.declaredBuildOnlyPackages);
  for (const packageName of duplicateBuild) {
    errors.push({ kind: 'invalid-external-policy-row', packageName, reason: 'duplicate-build-declaration' });
  }
  for (const packageName of [
    ...setDifference(expectedBuild, declaredBuild),
    ...setDifference(declaredBuild, expectedBuild),
  ]) {
    errors.push({ kind: 'invalid-external-policy-row', packageName, reason: 'invalid-build-declaration' });
  }
  const missingBuild = [...declaredBuild]
    .filter((name) => !declaresPackage(packageJson.devDependencies, name))
    .sort();
  const wrongScope = [...declaredBuild]
    .filter((name) => declaresPackage(packageJson.dependencies, name))
    .sort();
  if (missingBuild.length > 0 || wrongScope.length > 0) {
    errors.push({ kind: 'build-declaration-set-mismatch', missing: missingBuild, wrongScope });
  }

  const seenRows = new Set<string>();
  for (const row of policy.uses) {
    const key = allowanceKey(row);
    let reason: string | null = null;
    if (seenRows.has(key)) reason = 'duplicate-row';
    else if (!FRONTEND_LAYERS.includes(row.sourceLayer)) reason = 'unknown-source-layer';
    else if (row.mode !== 'runtime' && row.mode !== 'type-only' && row.mode !== 'build-style') {
      reason = 'unsupported-mode';
    }
    else if (row.resourceKind !== 'module' && row.resourceKind !== 'stylesheet') {
      reason = 'unsupported-resource-kind';
    }
    else if (!canonicalPackageName(row.packageName)) reason = 'noncanonical-package-name';
    else if (row.canonicalSubpaths.length === 0
      || row.canonicalSubpaths.some((subpath) => !canonicalSubpath(subpath))) reason = 'invalid-subpath';
    else if (duplicates(row.canonicalSubpaths.map((subpath) => subpath ?? '<root>')).length > 0) {
      reason = 'duplicate-subpath';
    }
    else if ((row.mode === 'runtime' || row.mode === 'type-only')
      && !declaresPackage(packageJson.dependencies, row.packageName)) {
      reason = 'runtime-package-not-in-dependencies';
    }
    else if (row.mode === 'build-style'
      && !declaresPackage(packageJson.dependencies, row.packageName)
      && !(declaredBuild.has(row.packageName)
        && declaresPackage(packageJson.devDependencies, row.packageName))) {
      reason = 'build-package-not-declared';
    } else if (row.mode === 'build-style'
      && (row.resourceKind !== 'stylesheet'
        || row.consumerSourceFile === null
        || !canonicalConsumerPath(row.consumerSourceFile)
        || !row.consumerSourceFile.endsWith('.css')
        || !stylesheetLayers.has(row.consumerSourceFile))) reason = 'invalid-build-style-consumer';
    else if (row.mode !== 'build-style' && row.consumerSourceFile !== null) reason = 'unexpected-consumer';
    if (reason !== null) {
      errors.push({ kind: 'invalid-external-policy-row', packageName: row.packageName, reason });
    }
    seenRows.add(key);
  }
  return errors;
}

function externalDependencies(
  dependencies: readonly ResolvedFrontendDependency[],
): ResolvedFrontendDependency[] {
  return dependencies
    .filter((dependency) => dependency.origin.kind === 'external')
    .sort((left, right) => [
      left.repositoryRelativeSourceFile,
      left.fullyQualifiedOwner,
      left.kind,
      left.canonicalOriginTarget,
      'location' in left ? left.location.line : left.line,
      'location' in left ? left.location.column : left.column,
    ].join('\u0000').localeCompare([
      right.repositoryRelativeSourceFile,
      right.fullyQualifiedOwner,
      right.kind,
      right.canonicalOriginTarget,
      'location' in right ? right.location.line : right.line,
      'location' in right ? right.location.column : right.column,
    ].join('\u0000')));
}

function modeRule(mode: FrontendDependencyMode, suffix: string): string {
  return `frontend.external.${mode}-${suffix}`;
}

function findingRule(
  dependency: ResolvedFrontendDependency,
  sourceLayer: FrontendLayer,
  consumerSourceFile: string | null,
  policy: ExternalDependencyPolicy,
): string | null {
  if (dependency.origin.kind !== 'external') return null;
  const origin = dependency.origin.dependency;
  const packageModeRows = policy.uses.filter((row) => (
    row.packageName === origin.packageName && row.mode === dependency.mode
  ));
  const layerRows = packageModeRows.filter((row) => row.sourceLayer === sourceLayer);
  if (layerRows.length === 0) return modeRule(dependency.mode, 'source-layer');
  const resourceRows = layerRows.filter((row) => row.resourceKind === origin.resourceKind);
  if (resourceRows.length === 0) {
    return dependency.mode === 'runtime'
      ? 'frontend.external.runtime-resource-kind'
      : modeRule(dependency.mode, 'subpath');
  }
  const subpathRows = resourceRows.filter((row) => row.canonicalSubpaths.includes(origin.canonicalSubpath));
  if (subpathRows.length === 0) return modeRule(dependency.mode, 'subpath');
  if (!subpathRows.some((row) => row.consumerSourceFile === consumerSourceFile)) {
    return dependency.mode === 'build-style'
      ? 'frontend.external.build-style-consumer'
      : modeRule(dependency.mode, 'subpath');
  }
  return null;
}

function asFinding(
  dependency: ResolvedFrontendDependency,
  sourceLayer: FrontendLayer,
  ruleId: string,
): FrontendFinding {
  const importedSymbol = 'importedSymbol' in dependency ? dependency.importedSymbol : null;
  const location = 'location' in dependency
    ? dependency.location
    : { line: dependency.line, column: dependency.column };
  return {
    ruleId,
    repositoryRelativeSourceFile: dependency.repositoryRelativeSourceFile,
    fullyQualifiedOwner: dependency.fullyQualifiedOwner,
    dependencyKind: dependency.kind,
    canonicalOriginTarget: dependency.canonicalOriginTarget,
    sourceLayer,
    targetLayer: null,
    importedSymbol,
    ...location,
  };
}

export function auditFrontendExternalDependencies(
  dependencies: readonly ResolvedFrontendDependency[],
  classification: ReadonlyMap<string, FrontendLayer>,
  stylesheetLayers: ReadonlyMap<string, FrontendLayer>,
  packageJson: ReadonlyPackageManifest,
  policy: ExternalDependencyPolicy,
): FrontendExternalDependencyReport {
  const errors = validatePolicy(packageJson, policy, stylesheetLayers);
  const findings: FrontendFinding[] = [];
  const evaluated: FrontendExternalDependencyEvaluation[] = [];
  for (const dependency of externalDependencies(dependencies)) {
    if (dependency.origin.kind !== 'external') continue;
    const sourceLayer = dependency.mode === 'build-style'
      ? stylesheetLayers.get(dependency.repositoryRelativeSourceFile)
      : classification.get(dependency.repositoryRelativeSourceFile);
    if (!sourceLayer) continue;
    const packageName = dependency.origin.dependency.packageName;
    let declarationScope: 'production' | 'development' | null = null;
    if (declaresPackage(packageJson.dependencies, packageName)) {
      declarationScope = 'production';
    } else if (dependency.mode === 'build-style'
      && policy.declaredBuildOnlyPackages.includes(packageName)
      && declaresPackage(packageJson.devDependencies, packageName)) {
      declarationScope = 'development';
    } else if (declaresPackage(packageJson.devDependencies, packageName)) {
      errors.push({
        kind: 'development-dependency-in-production',
        packageName,
        sourceFile: dependency.repositoryRelativeSourceFile,
      });
    } else {
      errors.push({
        kind: 'unknown-external-package',
        packageName,
        sourceFile: dependency.repositoryRelativeSourceFile,
      });
    }
    if (declarationScope === null) continue;
    const consumerSourceFile = dependency.mode === 'build-style'
      ? dependency.repositoryRelativeSourceFile
      : null;
    const ruleId = findingRule(dependency, sourceLayer, consumerSourceFile, policy);
    evaluated.push({
      sourceFile: dependency.repositoryRelativeSourceFile,
      sourceLayer,
      mode: dependency.mode,
      packageName,
      canonicalSubpath: dependency.origin.dependency.canonicalSubpath,
      resourceKind: dependency.origin.dependency.resourceKind,
      consumerSourceFile,
      declarationScope,
      allowed: ruleId === null,
    });
    if (ruleId !== null) findings.push(asFinding(dependency, sourceLayer, ruleId));
  }
  return { findings, errors, evaluated };
}
