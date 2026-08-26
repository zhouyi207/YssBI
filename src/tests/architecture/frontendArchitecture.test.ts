import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { ArchitectureSource } from '@/tests/helpers/moduleDependencyAudit';
import {
  closeTypeScriptAuditResources,
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
} from '@/tests/helpers/typescriptAudit';
import {
  ModuleDependencyResolutionError,
  createRepositoryTextReader,
  productionTypeScriptSources,
  resolvedModuleDependencies,
  resolvedStylesheetDependencies,
  type RepositoryTextReader,
  type FrontendFinding,
  type AssetDependencyPolicy,
  type ExternalDependencyPolicy,
  type ReadonlyPackageManifest,
  type ResolvedModuleDependency,
  type ResolvedStylesheetDependency,
  type ResolvedStylesheetGraph,
} from './frontendArchitectureModel';
import {
  classifyFrontendSources,
  type FrontendBaseRule,
  type FrontendLayer,
  type FrontendLiteralPolicyMembership,
} from './frontendArchitecturePolicy';
import {
  FRONTEND_ASSET_DEPENDENCY_POLICY,
  auditFrontendAssetDependencies,
} from './frontendAssetDependencyPolicy';
import {
  FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
  auditFrontendExternalDependencies,
} from './frontendExternalDependencyPolicy';
import {
  compareExactFrontendDebt,
  type FrontendDebtEntry,
} from './frontendArchitectureDebt';
import { FRONTEND_ARCHITECTURE_POLICY } from './frontendArchitecturePolicy';

class FixtureTextReader implements RepositoryTextReader {
  constructor(private readonly files: ReadonlyMap<string, string>) {}

  readRepositoryText(repositoryRelativePath: string): string | null {
    return this.files.get(repositoryRelativePath) ?? null;
  }
}

const compilerSources = new Map<string, string>([
  ['src/views/fixture.tsx', `
    import { approvedRead } from '../features/core/fixture/read';
    import type { Contract } from '../features/domain/fixture/contract';
    import { runtimeValue, type MixedContract } from '../features/domain/fixture/mixed';
    import type { RenamedContract } from '../features/domain/fixture/aliased-barrel';
    import type { ReactNode } from 'react';
    import type { Selection } from 'd3';
    import type { ReactTypesOnly } from '@types/react';
    import type { D3TypesOnly } from '@types/d3';
    import { createRoot } from 'react-dom/client';
    import { getCurrentWindow } from '@tauri-apps/api/window';
    import 'dockview-react/dist/styles/dockview.css';
    import 'katex/dist/katex.min.css';
    import './fixture.css';
    import RuntimeEquals = require('./runtime');

    export { approvedRead as reexportedRead } from '../features/core/fixture/read';
    export type { Contract as ReexportedContract } from '../features/domain/fixture/contract-barrel';

    type ImportedContract = import('../features/domain/fixture/contract-barrel').Contract;
    const loaded = import('./runtime');
    const required = require('./runtime');
    void [approvedRead, runtimeValue, createRoot, getCurrentWindow, RuntimeEquals, loaded, required];
    export type FixtureTypes = Contract | MixedContract | RenamedContract | ReactNode
      | Selection | ReactTypesOnly | D3TypesOnly | ImportedContract;
  `],
  ['src/views/export-assignment.ts', "export = require('./runtime');"],
  ['src/views/runtime.ts', 'export const runtimeValue = 1;'],
  ['src/features/core/fixture/read.ts', 'export const approvedRead = 1;'],
  ['src/features/domain/fixture/contract.ts', 'export interface Contract { readonly value: string; }'],
  ['src/features/domain/fixture/contract-barrel.ts', "export { Contract } from './contract';"],
  ['src/features/domain/fixture/aliased-barrel.ts', "export { Contract as RenamedContract } from './contract-barrel';"],
  ['src/features/domain/fixture/mixed.ts', `
    export const runtimeValue = 1;
    export interface MixedContract { readonly value: number; }
  `],
  ['src/app/i18n-facade.ts', `
    import i18n from 'i18next';
    export { i18n };
  `],
  ['src/features/application/i18n-user.ts', `
    import { i18n } from '../../app/i18n-facade';
    void i18n;
  `],
  ['node_modules/@types/react/index.d.ts', `
    export interface ReactNode { readonly reactNode: unique symbol; }
    export interface ReactTypesOnly { readonly forbiddenReact: unique symbol; }
  `],
  ['node_modules/@types/d3/index.d.ts', `
    export interface Selection { readonly selection: unique symbol; }
    export interface D3TypesOnly { readonly forbiddenD3: unique symbol; }
  `],
  ['node_modules/react-dom/client.d.ts', 'export declare function createRoot(): void;'],
  ['node_modules/@tauri-apps/api/window.d.ts', 'export declare function getCurrentWindow(): void;'],
  ['node_modules/i18next/index.d.ts', 'declare const i18n: { readonly language: string }; export default i18n;'],
  ['src/globals.d.ts', 'declare function require(specifier: string): unknown;'],
]);

const stylesheetSources = new Map<string, string>([
  ['src/views/fixture.css', `
    @import "tailwindcss";
    @import "tw-animate-css";
    @import "shadcn/tailwind.css";
    @import "@fontsource-variable/inter";
    @import "./nested.css";
    @import "./cycle-a.css";
    @import "./malformed.css";
    @import "./missing.css";
    @import "../../../outside.css";
    @import "../parent.css";
    @import "react\\secret";
    @import "react/%2fsecret";
    @import "https://example.invalid/theme.css";
    @import "./font.woff2";
    @import url(var(--theme));
    .fixture { background-image: url("./url-target.css"); }
  `],
  ['src/views/nested.css', '.nested { color: red; }'],
  ['src/views/url-target.css', '.url-target { color: blue; }'],
  ['src/views/cycle-a.css', '@import "./cycle-b.css";'],
  ['src/views/cycle-b.css', '@import "./cycle-a.css";'],
  ['src/views/malformed.css', '@import "unterminated.css;'],
]);

function architectureSource(path: string): ArchitectureSource {
  const source = compilerSources.get(path);
  if (source === undefined) throw new Error(`Missing compiler fixture ${path}`);
  return { path, source };
}

function stylesheetRoot(path: string): ResolvedModuleDependency {
  const fileName = path.split('/').slice(-1)[0];
  return {
    kind: 'side-effect-import',
    mode: 'runtime',
    specifier: `./${fileName}`,
    location: { line: 1, column: 1 },
    repositoryRelativeSourceFile: 'src/views/fixture.ts',
    fullyQualifiedOwner: 'src/views/fixture.ts::<module>',
    origin: {
      kind: 'repository-asset',
      asset: { repositoryRelativeAssetPath: path, resourceKind: 'stylesheet' },
    },
    canonicalOriginTarget: `repository-asset:${path}`,
    importedSymbol: null,
    writtenModuleSpecifier: `./${fileName}`,
    symbolDeclarationTarget: null,
  };
}

function externalModuleDependency(
  sourceFile: string,
  packageName: string,
  canonicalSubpath: string | null,
  mode: 'runtime' | 'type-only',
  resourceKind: 'module' | 'stylesheet' = 'module',
  symbolDeclarationTarget: string | null = null,
): ResolvedModuleDependency {
  const writtenSubpath = canonicalSubpath?.split('::').join('/');
  const writtenModuleSpecifier = `${packageName}${writtenSubpath ? `/${writtenSubpath}` : ''}`;
  return {
    kind: resourceKind === 'stylesheet' ? 'side-effect-import' : 'static-import',
    mode,
    specifier: writtenModuleSpecifier,
    location: { line: 1, column: 1 },
    repositoryRelativeSourceFile: sourceFile,
    fullyQualifiedOwner: `${sourceFile}::<module>`,
    origin: {
      kind: 'external',
      dependency: { packageName, canonicalSubpath, resourceKind },
    },
    canonicalOriginTarget: `external:${packageName}${canonicalSubpath ? `::${canonicalSubpath}` : ''}`,
    importedSymbol: resourceKind === 'module' ? 'fixture' : null,
    writtenModuleSpecifier,
    symbolDeclarationTarget,
  };
}

function repositoryAssetDependency(
  sourceFile: string,
  assetPath: string,
): ResolvedModuleDependency {
  const assetFileName = assetPath.split('/').slice(-1)[0];
  return {
    kind: 'side-effect-import',
    mode: 'runtime',
    specifier: `./${assetFileName}`,
    location: { line: 1, column: 1 },
    repositoryRelativeSourceFile: sourceFile,
    fullyQualifiedOwner: `${sourceFile}::<module>`,
    origin: {
      kind: 'repository-asset',
      asset: { repositoryRelativeAssetPath: assetPath, resourceKind: 'stylesheet' },
    },
    canonicalOriginTarget: `repository-asset:${assetPath}`,
    importedSymbol: null,
    writtenModuleSpecifier: `./${assetFileName}`,
    symbolDeclarationTarget: null,
  };
}

function stylesheetExternalDependency(
  sourceFile: string,
  packageName: string,
  canonicalSubpath: string | null,
): ResolvedStylesheetDependency {
  const writtenSubpath = canonicalSubpath?.split('::').join('/');
  const writtenSpecifier = `${packageName}${writtenSubpath ? `/${writtenSubpath}` : ''}`;
  return {
    repositoryRelativeSourceFile: sourceFile,
    fullyQualifiedOwner: `stylesheet:${sourceFile}`,
    kind: 'stylesheet-import',
    mode: 'build-style',
    origin: {
      kind: 'external',
      dependency: { packageName, canonicalSubpath, resourceKind: 'stylesheet' },
    },
    canonicalOriginTarget: `external:${packageName}${canonicalSubpath ? `::${canonicalSubpath}` : ''}`,
    writtenSpecifier,
    line: 1,
    column: 1,
  };
}

function stylesheetAssetDependency(
  sourceFile: string,
  assetPath: string,
): ResolvedStylesheetDependency {
  return {
    repositoryRelativeSourceFile: sourceFile,
    fullyQualifiedOwner: `stylesheet:${sourceFile}`,
    kind: 'stylesheet-import',
    mode: 'build-style',
    origin: {
      kind: 'repository-asset',
      asset: { repositoryRelativeAssetPath: assetPath, resourceKind: 'stylesheet' },
    },
    canonicalOriginTarget: `repository-asset:${assetPath}`,
    writtenSpecifier: `./${assetPath.split('/').slice(-1)[0]}`,
    line: 1,
    column: 1,
  };
}

function architectureFinding(
  overrides: Partial<FrontendFinding> = {},
): FrontendFinding {
  return {
    ruleId: 'frontend.layer.views-target',
    repositoryRelativeSourceFile: 'src/views/fixture.ts',
    fullyQualifiedOwner: 'src/views/fixture.ts::<module>',
    dependencyKind: 'static-import',
    canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
    sourceLayer: 'views',
    targetLayer: 'domain',
    importedSymbol: 'Contract',
    line: 1,
    column: 1,
    ...overrides,
  };
}

describe('frontend architecture model', () => {
  it('classifies every frontend production source exactly once', () => {
    const emptyMembership = Object.fromEntries([
      'app-composition',
      'views',
      'application',
      'core',
      'domain',
      'services',
      'components-ui',
      'wire-schema',
      'diagnostics',
      'pure-shared',
    ].map((layer) => [layer, []])) as unknown as Record<FrontendLayer, readonly string[]>;
    const literalMembership: FrontendLiteralPolicyMembership = {
      ...emptyMembership,
      services: ['src/shared/platform/testAdapter.ts'],
      core: ['src/shared/overlap.ts'],
      diagnostics: ['src/shared/overlap.ts'],
    };
    const report = classifyFrontendSources([
      { path: 'src\\app\\fixture.ts', source: 'export const fixture = true;' },
      { path: 'src/shared/platform/testAdapter.ts', source: 'export const adapter = true;' },
      { path: 'src/unowned/fixture.ts', source: 'export const fixture = true;' },
      { path: 'src/shared/overlap.ts', source: 'export const overlap = true;' },
    ], literalMembership);

    expect([...report.classification]).toEqual([
      ['src/app/fixture.ts', 'app-composition'],
      ['src/shared/platform/testAdapter.ts', 'services'],
    ]);
    expect(report.errors).toEqual([
      {
        kind: 'multiply-classified-production-source',
        sourceFile: 'src/shared/overlap.ts',
        layers: ['core', 'diagnostics'],
      },
      {
        kind: 'unclassified-production-source',
        sourceFile: 'src/unowned/fixture.ts',
      },
    ]);

    withProductionTypeScriptProject((context) => {
      const productionReport = classifyFrontendSources(productionTypeScriptSources(context));
      expect(productionReport.errors).toEqual([]);
      expect(productionReport.classification.size).toBe(productionTypeScriptSources(context).length);
    });
    expect(FRONTEND_ARCHITECTURE_POLICY.capabilities).toContainEqual(expect.objectContaining({
      canonicalModule: 'src/features/core/dockview/workbenchDockviewPort.ts',
      exportedSymbols: ['WorkbenchDockviewPort'],
      memberCapabilities: {
        WorkbenchDockviewRead: [
          'isReady',
          'isHydrated',
          'whenHydrated',
          'subscribe',
          'getSnapshot',
          'getPanel',
          'getActivePanel',
          'getActiveEditorPanel',
          'listPanels',
          'listGroups',
          'listGroupPanels',
          'findEditorPanelsByResource',
          'getEdgeState',
        ],
      },
    }));
    expect(FRONTEND_ARCHITECTURE_POLICY.capabilities.filter((capability) => (
      capability.canonicalModule === 'src/features/core/dockview/workbenchDockviewPort.ts'
      && capability.exportedSymbols.includes('WorkbenchDockviewPort')
    )).map(({ sourceLayer }) => sourceLayer)).toEqual(['app-composition', 'views']);
  });

  it('reports overlapping frontend base memberships without rule ordering', () => {
    const emptyMembership = Object.fromEntries([
      'app-composition',
      'views',
      'application',
      'core',
      'domain',
      'services',
      'components-ui',
      'wire-schema',
      'diagnostics',
      'pure-shared',
    ].map((layer) => [layer, []])) as unknown as Record<FrontendLayer, readonly string[]>;
    const baseRules: readonly FrontendBaseRule[] = [
      { layer: 'views', matches: (path) => path.endsWith('/base-overlap.ts') },
      { layer: 'application', matches: (path) => path.startsWith('src/base-') },
    ];

    const report = classifyFrontendSources([
      { path: 'src/base-overlap.ts', source: 'export const overlap = true;' },
    ], emptyMembership, baseRules);

    expect([...report.classification]).toEqual([]);
    expect(report.errors).toEqual([{
      kind: 'multiply-classified-production-source',
      sourceFile: 'src/base-overlap.ts',
      layers: ['views', 'application'],
    }]);
  });

  it('audits frontend packages and stylesheet assets by layer mode and origin', () => {
    const productionSources = [
      'src/app/App.tsx',
      'src/app/main.tsx',
      'src/views/fixture.tsx',
      'src/features/application/fixture.ts',
      'src/features/core/fixture.ts',
      'src/features/domain/fixture.ts',
      'src/services/fixture.ts',
      'src/components/fixture.tsx',
    ].map((path) => ({ path, source: 'export {};' }));
    const classification = classifyFrontendSources(productionSources).classification;
    const moduleDependencies = [
      externalModuleDependency('src/views/fixture.tsx', 'react', null, 'runtime'),
      externalModuleDependency(
        'src/views/fixture.tsx',
        'react',
        null,
        'type-only',
        'module',
        'node_modules/@types/react/index.d.ts::ReactNode',
      ),
      externalModuleDependency(
        'src/views/fixture.tsx',
        'd3',
        null,
        'type-only',
        'module',
        'node_modules/@types/d3/index.d.ts::Selection',
      ),
      externalModuleDependency('src/features/application/fixture.ts', 'zustand', 'react::shallow', 'runtime'),
      externalModuleDependency('src/features/core/fixture.ts', 'dockview-react', null, 'type-only'),
      externalModuleDependency('src/services/fixture.ts', '@tauri-apps/api', 'core', 'runtime'),
      externalModuleDependency('src/app/App.tsx', 'dockview-react', 'dist::styles::dockview.css', 'runtime', 'stylesheet'),
      externalModuleDependency('src/views/fixture.tsx', 'katex', 'dist::katex.min.css', 'runtime', 'stylesheet'),
      externalModuleDependency('src/components/fixture.tsx', 'katex', 'dist::katex.min.css', 'runtime', 'stylesheet'),
      repositoryAssetDependency('src/app/App.tsx', 'src/app/App.css'),
      repositoryAssetDependency('src/app/main.tsx', 'src/app/workbench-dockview.css'),
      externalModuleDependency('src/features/domain/fixture.ts', 'react', null, 'runtime'),
      externalModuleDependency('src/views/fixture.tsx', 'zustand', null, 'runtime'),
      externalModuleDependency('src/views/fixture.tsx', '@tauri-apps/api', 'window', 'runtime'),
      externalModuleDependency('src/features/application/fixture.ts', 'dockview-react', null, 'runtime'),
      externalModuleDependency('src/views/fixture.tsx', 'react', 'unlisted', 'runtime'),
      externalModuleDependency('src/app/App.tsx', 'dockview-react', 'dist::styles::other.css', 'runtime', 'stylesheet'),
      externalModuleDependency('src/views/fixture.tsx', 'dockview-react', 'dist::styles::dockview.css', 'runtime', 'stylesheet'),
      repositoryAssetDependency('src/views/fixture.tsx', 'src/app/App.css'),
      externalModuleDependency('src/features/application/fixture.ts', 'tailwindcss', null, 'runtime'),
      externalModuleDependency('src/features/application/fixture.ts', 'tailwindcss', null, 'type-only'),
      externalModuleDependency('src/views/fixture.tsx', '@types/react', null, 'type-only'),
      externalModuleDependency('src/views/fixture.tsx', '@types/d3', null, 'type-only'),
      externalModuleDependency('src/views/fixture.tsx', 'vitest', null, 'runtime'),
      externalModuleDependency('src/views/fixture.tsx', 'mystery-package', null, 'runtime'),
      externalModuleDependency('src/views/fixture.tsx', 'toString', null, 'runtime'),
    ];
    const stylesheetDependencies = [
      stylesheetExternalDependency('src/app/App.css', 'tailwindcss', null),
      stylesheetExternalDependency('src/app/App.css', 'tw-animate-css', null),
      stylesheetExternalDependency('src/app/App.css', 'shadcn', 'tailwind.css'),
      stylesheetExternalDependency('src/app/App.css', '@fontsource-variable/inter', null),
      stylesheetExternalDependency('src/app/workbench-dockview.css', 'tailwindcss', null),
    ];
    const stylesheetGraph: ResolvedStylesheetGraph = {
      repositoryStylesheets: ['src/app/App.css', 'src/app/workbench-dockview.css'],
      dependencies: stylesheetDependencies,
      errors: [
        { kind: 'stylesheet-target-missing', sourceFile: 'src/app/App.css', canonicalTarget: 'src/app/missing.css' },
        { kind: 'stylesheet-path-escapes-repository', sourceFile: 'src/app/App.css', writtenSpecifier: '../../../outside.css' },
        { kind: 'unsupported-stylesheet-target', sourceFile: 'src/app/App.css', writtenSpecifier: './font.woff2' },
        { kind: 'stylesheet-cycle', cycle: ['src/app/a.css', 'src/app/b.css', 'src/app/a.css'] },
      ],
    };
    const packageJson = JSON.parse(readFileSync('package.json', 'utf8')) as ReadonlyPackageManifest;

    expect(FRONTEND_EXTERNAL_DEPENDENCY_POLICY.declaredRuntimePackages).toEqual(
      Object.keys(packageJson.dependencies).sort(),
    );
    expect(FRONTEND_EXTERNAL_DEPENDENCY_POLICY.declaredBuildOnlyPackages).toEqual(['tailwindcss']);
    expect(FRONTEND_ASSET_DEPENDENCY_POLICY.uses).toEqual([
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
    ]);

    const assetReport = auditFrontendAssetDependencies({
      productionSources,
      moduleDependencies,
      stylesheetGraph,
    }, classification, FRONTEND_ASSET_DEPENDENCY_POLICY);
    expect(assetReport.stylesheetLayers).toEqual(new Map([
      ['src/app/App.css', 'app-composition'],
      ['src/app/workbench-dockview.css', 'app-composition'],
    ]));
    expect(assetReport.findings).toEqual([
      expect.objectContaining({
        ruleId: 'frontend.asset.consumer-path',
        repositoryRelativeSourceFile: 'src/views/fixture.tsx',
        canonicalOriginTarget: 'repository-asset:src/app/App.css',
      }),
    ]);
    expect(assetReport.errors).toEqual(stylesheetGraph.errors);

    const externalReport = auditFrontendExternalDependencies(
      [...moduleDependencies, ...stylesheetDependencies],
      classification,
      assetReport.stylesheetLayers,
      packageJson,
      FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
    );
    expect(externalReport.evaluated).toEqual(expect.arrayContaining([
      expect.objectContaining({ packageName: 'react', mode: 'type-only', declarationScope: 'production', allowed: true }),
      expect.objectContaining({ packageName: 'd3', mode: 'type-only', declarationScope: 'production', allowed: true }),
      expect.objectContaining({ packageName: 'tailwindcss', mode: 'build-style', declarationScope: 'development', allowed: true }),
      expect.objectContaining({ packageName: '@tauri-apps/api', canonicalSubpath: 'core', allowed: true }),
    ]));
    expect(externalReport.findings.map(({ ruleId }) => ruleId)).toEqual([
      'frontend.external.runtime-subpath',
      'frontend.external.build-style-consumer',
      'frontend.external.runtime-source-layer',
      'frontend.external.runtime-source-layer',
      'frontend.external.runtime-resource-kind',
      'frontend.external.runtime-source-layer',
      'frontend.external.runtime-subpath',
      'frontend.external.runtime-subpath',
    ]);
    expect(externalReport.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ kind: 'development-dependency-in-production', packageName: 'tailwindcss' }),
      expect.objectContaining({ kind: 'development-dependency-in-production', packageName: '@types/react' }),
      expect.objectContaining({ kind: 'development-dependency-in-production', packageName: '@types/d3' }),
      expect.objectContaining({ kind: 'development-dependency-in-production', packageName: 'vitest' }),
      expect.objectContaining({ kind: 'unknown-external-package', packageName: 'mystery-package' }),
      expect.objectContaining({ kind: 'unknown-external-package', packageName: 'toString' }),
    ]));
    expect(externalReport.errors.filter(({ kind }) => (
      kind === 'development-dependency-in-production'
    ))).toHaveLength(5);

    const duplicateExternalPolicy = {
      ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
      uses: [
        ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses,
        FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses[0],
      ],
    };
    expect(auditFrontendExternalDependencies(
      [],
      classification,
      assetReport.stylesheetLayers,
      packageJson,
      duplicateExternalPolicy,
    ).errors).toContainEqual(expect.objectContaining({ kind: 'invalid-external-policy-row' }));
    const duplicateSubpathPolicy = {
      ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
      uses: FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses.map((row, index) => (
        index === 0 ? { ...row, canonicalSubpaths: [null, null] } : row
      )),
    };
    expect(auditFrontendExternalDependencies(
      [],
      classification,
      assetReport.stylesheetLayers,
      packageJson,
      duplicateSubpathPolicy,
    ).errors).toContainEqual(expect.objectContaining({
      kind: 'invalid-external-policy-row',
      reason: 'duplicate-subpath',
    }));
    const unsupportedModePolicy = {
      ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
      uses: FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses.map((row, index) => (
        index === 0 ? { ...row, mode: 'test-only' } : row
      )),
    } as unknown as ExternalDependencyPolicy;
    expect(auditFrontendExternalDependencies(
      [],
      classification,
      assetReport.stylesheetLayers,
      packageJson,
      unsupportedModePolicy,
    ).errors).toContainEqual(expect.objectContaining({
      kind: 'invalid-external-policy-row',
      reason: 'unsupported-mode',
    }));
    const missingBuildConsumerPolicy = {
      ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
      uses: FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses.map((row) => (
        row.mode === 'build-style' && row.packageName === 'tailwindcss'
          ? { ...row, consumerSourceFile: 'src/app/missing.css' }
          : row
      )),
    };
    expect(auditFrontendExternalDependencies(
      [],
      classification,
      assetReport.stylesheetLayers,
      packageJson,
      missingBuildConsumerPolicy,
    ).errors).toContainEqual(expect.objectContaining({
      kind: 'invalid-external-policy-row',
      reason: 'invalid-build-style-consumer',
    }));
    const duplicateAssetPolicy = {
      uses: [...FRONTEND_ASSET_DEPENDENCY_POLICY.uses, FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0]],
    };
    expect(auditFrontendAssetDependencies({
      productionSources,
      moduleDependencies,
      stylesheetGraph: { ...stylesheetGraph, errors: [] },
    }, classification, duplicateAssetPolicy).errors).toContainEqual(
      expect.objectContaining({ kind: 'invalid-asset-policy-row' }),
    );
    const typeOnlyAssetPolicy = {
      uses: [{ ...FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0], mode: 'type-only' }],
    } as unknown as AssetDependencyPolicy;
    expect(auditFrontendAssetDependencies({
      productionSources,
      moduleDependencies,
      stylesheetGraph: { ...stylesheetGraph, errors: [] },
    }, classification, typeOnlyAssetPolicy).errors).toContainEqual(expect.objectContaining({
      kind: 'invalid-asset-policy-row',
      reason: 'unsupported-mode',
    }));
    const stylesheetRuntimeConsumerPolicy: AssetDependencyPolicy = {
      uses: [{
        ...FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0],
        consumerSourceFile: 'src/app/App.css',
      }],
    };
    expect(auditFrontendAssetDependencies({
      productionSources,
      moduleDependencies,
      stylesheetGraph: { ...stylesheetGraph, errors: [] },
    }, classification, stylesheetRuntimeConsumerPolicy).errors).toContainEqual(expect.objectContaining({
      kind: 'invalid-asset-policy-row',
      reason: 'runtime-asset-consumer-not-typescript',
    }));

    const conflictModules = [
      repositoryAssetDependency('src/app/App.tsx', 'src/app/shared.css'),
      repositoryAssetDependency('src/views/fixture.tsx', 'src/app/shared.css'),
    ];
    const conflictPolicy: AssetDependencyPolicy = {
      uses: [
        {
          ...FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0],
          repositoryRelativeAssetPath: 'src/app/shared.css',
        },
        {
          ...FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0],
          sourceLayer: 'views',
          consumerSourceFile: 'src/views/fixture.tsx',
          repositoryRelativeAssetPath: 'src/app/shared.css',
        },
      ],
    };
    const conflictReport = auditFrontendAssetDependencies({
      productionSources,
      moduleDependencies: conflictModules,
      stylesheetGraph: {
        repositoryStylesheets: ['src/app/shared.css'],
        dependencies: [],
        errors: [],
      },
    }, classification, conflictPolicy);
    expect(conflictReport.stylesheetLayers.has('src/app/shared.css')).toBe(false);
    expect(conflictReport.errors).toContainEqual({
      kind: 'stylesheet-layer-conflict',
      sourceFile: 'src/app/shared.css',
      inheritedLayers: ['app-composition', 'views'],
    });
  });

  it('invalidates nested stylesheet provenance when a parent gains a second layer', () => {
    const productionSources = [
      { path: 'src/app/App.tsx', source: "import './shared.css';" },
      { path: 'src/views/fixture.tsx', source: "import '../app/shared.css';" },
    ];
    const classification = classifyFrontendSources(productionSources).classification;
    const moduleDependencies = [
      repositoryAssetDependency('src/app/App.tsx', 'src/app/shared.css'),
      repositoryAssetDependency('src/views/fixture.tsx', 'src/app/shared.css'),
    ];
    const policy: AssetDependencyPolicy = {
      uses: [
        {
          sourceLayer: 'app-composition',
          mode: 'runtime',
          dependencyKind: 'side-effect-import',
          resourceKind: 'stylesheet',
          consumerSourceFile: 'src/app/App.tsx',
          repositoryRelativeAssetPath: 'src/app/shared.css',
        },
        {
          sourceLayer: 'views',
          mode: 'runtime',
          dependencyKind: 'side-effect-import',
          resourceKind: 'stylesheet',
          consumerSourceFile: 'src/views/fixture.tsx',
          repositoryRelativeAssetPath: 'src/app/shared.css',
        },
        {
          sourceLayer: 'app-composition',
          mode: 'build-style',
          dependencyKind: 'stylesheet-import',
          resourceKind: 'stylesheet',
          consumerSourceFile: 'src/app/shared.css',
          repositoryRelativeAssetPath: 'src/app/nested.css',
        },
      ],
    };

    const report = auditFrontendAssetDependencies({
      productionSources,
      moduleDependencies,
      stylesheetGraph: {
        repositoryStylesheets: ['src/app/nested.css', 'src/app/shared.css'],
        dependencies: [
          stylesheetAssetDependency('src/app/shared.css', 'src/app/nested.css'),
        ],
        errors: [],
      },
    }, classification, policy);

    expect([...report.stylesheetLayers]).toEqual([]);
    expect(report.findings).toEqual([]);
    expect(report.errors).toEqual([{
      kind: 'stylesheet-layer-conflict',
      sourceFile: 'src/app/shared.css',
      inheritedLayers: ['app-composition', 'views'],
    }]);
  });

  it('ratchets frontend debt in both directions', () => {
    const actual = [
      architectureFinding({ line: 10, column: 2 }),
      architectureFinding({ line: 20, column: 4 }),
      architectureFinding({
        dependencyKind: 'import-type',
        canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
      }),
      architectureFinding({
        dependencyKind: 'dynamic-import',
        canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
      }),
    ];
    const declared: FrontendDebtEntry[] = [
      {
        ruleId: 'frontend.layer.views-target',
        repositoryRelativeSourceFile: 'src/views/fixture.ts',
        fullyQualifiedOwner: 'src/views/fixture.ts::<module>',
        dependencyKind: 'static-import',
        canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
        expectedOccurrences: 1,
        owningMigrationSpec: 'docs/architecture/FRONTEND_APPLICATION_BOUNDARIES.md',
      },
      {
        ruleId: 'frontend.layer.views-target',
        repositoryRelativeSourceFile: 'src/views/moved.ts',
        fullyQualifiedOwner: 'src/views/moved.ts::<module>',
        dependencyKind: 'static-import',
        canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
        expectedOccurrences: 3,
        owningMigrationSpec: 'docs/architecture/PROJECT_GRAPH_OWNERSHIP_BOUNDARIES.md',
      },
      {
        ruleId: 'frontend.layer.views-target',
        repositoryRelativeSourceFile: 'src/views/fixture.ts',
        fullyQualifiedOwner: 'src/views/fixture.ts::<module>',
        dependencyKind: 'dynamic-import',
        canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
        expectedOccurrences: 1,
        owningMigrationSpec: 'docs/architecture/EXECUTION_RUNTIME_BOUNDARIES.md',
      },
    ];
    expect(Object.keys(declared[0]).sort()).toEqual([
      'canonicalOriginTarget',
      'dependencyKind',
      'expectedOccurrences',
      'fullyQualifiedOwner',
      'owningMigrationSpec',
      'repositoryRelativeSourceFile',
      'ruleId',
    ]);

    const mismatch = compareExactFrontendDebt(actual, declared);
    expect(mismatch.errors).toEqual([]);
    expect(mismatch.newOrIncreased).toContainEqual(expect.objectContaining({
      dependencyKind: 'static-import',
      actualOccurrences: 2,
      expectedOccurrences: 1,
    }));
    expect(mismatch.newOrIncreased).toContainEqual(expect.objectContaining({
      dependencyKind: 'import-type',
      canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
      actualOccurrences: 1,
      expectedOccurrences: 0,
    }));
    expect(mismatch.newOrIncreased).not.toContainEqual(expect.objectContaining({
      dependencyKind: 'dynamic-import',
      canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
    }));
    expect(mismatch.staleOrDecreased).toEqual([
      expect.objectContaining({
        repositoryRelativeSourceFile: 'src/views/moved.ts',
        actualOccurrences: 0,
        expectedOccurrences: 3,
      }),
    ]);

    const duplicate = { ...declared[0] };
    const invalid = compareExactFrontendDebt([], [
      declared[0],
      duplicate,
      { ...declared[1], expectedOccurrences: 0 },
      { ...declared[2], owningMigrationSpec: 'docs/superpowers/unapproved.md' },
    ]);
    expect(invalid.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ kind: 'duplicate-frontend-debt-key' }),
      expect.objectContaining({ kind: 'invalid-frontend-debt-count', expectedOccurrences: 0 }),
      expect.objectContaining({
        kind: 'invalid-frontend-debt-owning-spec',
        owningMigrationSpec: 'docs/superpowers/unapproved.md',
      }),
    ]));
  });

  it('resolves every module dependency to its canonical origin', () => {
    withIsolatedTypeScriptProject(compilerSources, (context) => {
      const sourcePaths = [...compilerSources.keys()].filter((path) => (
        path.startsWith('src/') && !path.endsWith('.d.ts')
      ));
      const resolved = sourcePaths.flatMap((path) => (
        resolvedModuleDependencies(context, architectureSource(path))
      ));

      expect(resolved).toContainEqual(expect.objectContaining({
        repositoryRelativeSourceFile: 'src/views/fixture.tsx',
        importedSymbol: 'approvedRead',
        canonicalOriginTarget: 'src/features/core/fixture/read.ts::approvedRead',
      }));
      expect(resolved).toContainEqual(expect.objectContaining({
        kind: 'import-type',
        mode: 'type-only',
        importedSymbol: 'Contract',
        canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
      }));
      expect(resolved).toContainEqual(expect.objectContaining({
        kind: 'dynamic-import',
        mode: 'runtime',
        importedSymbol: null,
        canonicalOriginTarget: 'src/views/runtime.ts',
      }));
      expect(resolved.filter((dependency) => dependency.kind === 'import-type')).toHaveLength(1);
      expect(resolved.filter((dependency) => dependency.kind === 'dynamic-import')).toHaveLength(1);

      expect(resolved).toEqual(expect.arrayContaining([
        expect.objectContaining({ kind: 'require', mode: 'runtime', canonicalOriginTarget: 'src/views/runtime.ts' }),
        expect.objectContaining({ kind: 'import-equals', mode: 'runtime', canonicalOriginTarget: 'src/views/runtime.ts' }),
        expect.objectContaining({ kind: 'export-assignment', mode: 'runtime', canonicalOriginTarget: 'src/views/runtime.ts' }),
        expect.objectContaining({
          kind: 'static-import',
          mode: 'runtime',
          importedSymbol: 'runtimeValue',
          canonicalOriginTarget: 'src/features/domain/fixture/mixed.ts::runtimeValue',
        }),
        expect.objectContaining({
          kind: 'static-import',
          mode: 'type-only',
          importedSymbol: 'MixedContract',
          canonicalOriginTarget: 'src/features/domain/fixture/mixed.ts::MixedContract',
        }),
        expect.objectContaining({
          kind: 're-export',
          mode: 'type-only',
          importedSymbol: 'Contract',
          canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
        }),
        expect.objectContaining({
          kind: 'static-import',
          importedSymbol: 'Contract',
          canonicalOriginTarget: 'src/features/domain/fixture/contract.ts::Contract',
        }),
      ]));

      const repositoryDependencies = resolved.filter(({ origin }) => origin.kind === 'repository-module');
      expect(repositoryDependencies.every(({ origin, symbolDeclarationTarget }) => {
        if (origin.kind !== 'repository-module') return false;
        return origin.declarationTarget.startsWith('src/')
          && (symbolDeclarationTarget === null || symbolDeclarationTarget.startsWith('src/'));
      })).toBe(true);

      expect(resolved).toEqual(expect.arrayContaining([
        expect.objectContaining({
          mode: 'type-only',
          canonicalOriginTarget: 'external:react',
          symbolDeclarationTarget: 'node_modules/@types/react/index.d.ts::ReactNode',
          origin: {
            kind: 'external',
            dependency: { packageName: 'react', canonicalSubpath: null, resourceKind: 'module' },
          },
        }),
        expect.objectContaining({
          mode: 'type-only',
          canonicalOriginTarget: 'external:d3',
          symbolDeclarationTarget: 'node_modules/@types/d3/index.d.ts::Selection',
          origin: {
            kind: 'external',
            dependency: { packageName: 'd3', canonicalSubpath: null, resourceKind: 'module' },
          },
        }),
        expect.objectContaining({
          canonicalOriginTarget: 'external:@types/react',
          origin: {
            kind: 'external',
            dependency: { packageName: '@types/react', canonicalSubpath: null, resourceKind: 'module' },
          },
        }),
        expect.objectContaining({
          canonicalOriginTarget: 'external:@types/d3',
          origin: {
            kind: 'external',
            dependency: { packageName: '@types/d3', canonicalSubpath: null, resourceKind: 'module' },
          },
        }),
        expect.objectContaining({ canonicalOriginTarget: 'external:react-dom::client' }),
        expect.objectContaining({ canonicalOriginTarget: 'external:@tauri-apps/api::window' }),
        expect.objectContaining({
          repositoryRelativeSourceFile: 'src/features/application/i18n-user.ts',
          canonicalOriginTarget: 'external:i18next',
          symbolDeclarationTarget: 'node_modules/i18next/index.d.ts::i18n',
        }),
        expect.objectContaining({
          kind: 'side-effect-import',
          mode: 'runtime',
          canonicalOriginTarget: 'external:dockview-react::dist::styles::dockview.css',
          origin: {
            kind: 'external',
            dependency: {
              packageName: 'dockview-react',
              canonicalSubpath: 'dist::styles::dockview.css',
              resourceKind: 'stylesheet',
            },
          },
        }),
        expect.objectContaining({
          canonicalOriginTarget: 'external:katex::dist::katex.min.css',
        }),
        expect.objectContaining({
          canonicalOriginTarget: 'repository-asset:src/views/fixture.css',
          origin: {
            kind: 'repository-asset',
            asset: { repositoryRelativeAssetPath: 'src/views/fixture.css', resourceKind: 'stylesheet' },
          },
        }),
      ]));

      const stylesheetGraph = resolvedStylesheetDependencies(
        resolve('.'),
        resolved,
        new FixtureTextReader(stylesheetSources),
      );
      expect(stylesheetGraph.repositoryStylesheets).toEqual([
        'src/views/cycle-a.css',
        'src/views/cycle-b.css',
        'src/views/fixture.css',
        'src/views/malformed.css',
        'src/views/nested.css',
        'src/views/url-target.css',
      ]);
      expect(stylesheetGraph.dependencies).toEqual(expect.arrayContaining([
        ...[
          ['external:tailwindcss', 'tailwindcss'],
          ['external:tw-animate-css', 'tw-animate-css'],
          ['external:shadcn::tailwind.css', 'shadcn'],
          ['external:@fontsource-variable/inter', '@fontsource-variable/inter'],
        ].map(([canonicalOriginTarget, packageName]) => expect.objectContaining({
          repositoryRelativeSourceFile: 'src/views/fixture.css',
          fullyQualifiedOwner: 'stylesheet:src/views/fixture.css',
          kind: 'stylesheet-import',
          mode: 'build-style',
          canonicalOriginTarget,
          origin: expect.objectContaining({
            kind: 'external',
            dependency: expect.objectContaining({ packageName, resourceKind: 'stylesheet' }),
          }),
        })),
        expect.objectContaining({
          repositoryRelativeSourceFile: 'src/views/fixture.css',
          kind: 'stylesheet-url',
          mode: 'build-style',
          canonicalOriginTarget: 'repository-asset:src/views/url-target.css',
        }),
      ]));
      expect(stylesheetGraph.dependencies.every(({ line, column }) => line > 0 && column > 0)).toBe(true);
      expect(stylesheetGraph.errors.map(({ kind }) => kind)).toEqual(expect.arrayContaining([
        'stylesheet-parse-failure',
        'stylesheet-cycle',
        'stylesheet-path-escapes-repository',
        'stylesheet-target-missing',
        'unsupported-stylesheet-target',
      ]));
    });

    const invalidModuleSources = [
      ['src/views/nonliteral.ts', 'const target = getTarget(); void import(target);', 'nonliteral-module-specifier'],
      ['src/views/parent-package.ts', "import 'react/../secret';", 'invalid-external-specifier'],
      ['src/views/backslash-package.ts', "import 'react\\\\secret';", 'invalid-external-specifier'],
      ['src/views/encoded-package.ts', "import 'react/%2fsecret';", 'invalid-external-specifier'],
      ['src/views/missing-package.ts', "import 'not-a-real-package';", 'unresolved-module-dependency'],
    ] as const;
    withIsolatedTypeScriptProject(
      new Map(invalidModuleSources.map(([path, source]) => [path, source])),
      (context) => {
        for (const [path, source, kind] of invalidModuleSources) {
          try {
            resolvedModuleDependencies(context, { path, source });
            expect.fail(`Expected ${path} to fail closed`);
          } catch (error) {
            expect(error).toBeInstanceOf(ModuleDependencyResolutionError);
            expect(error).toMatchObject({ kind, sourceFile: path });
          }
        }
      },
    );

  });

  it('collects nested dependencies inside recognized import syntax', () => {
    const sources = new Map<string, string>([
      ['src/views/nested.ts', `
        export type Nested = import('./outer').Box<import('./inner').Thing>;
        void import('./outer', { with: { type: import('./inner') } });
        export default import('./outer', (import('./inner'), {}));
      `],
      ['src/views/outer.ts', 'export interface Box<T> { readonly value: T; }'],
      ['src/views/inner.ts', 'export interface Thing { readonly value: string; }'],
    ]);

    withIsolatedTypeScriptProject(sources, (context) => {
      const resolved = resolvedModuleDependencies(context, {
        path: 'src/views/nested.ts',
        source: sources.get('src/views/nested.ts')!,
      });

      expect(resolved.map(({ kind, canonicalOriginTarget }) => ({
        kind,
        canonicalOriginTarget,
      }))).toEqual([
        {
          kind: 'import-type',
          canonicalOriginTarget: 'src/views/outer.ts::Box',
        },
        {
          kind: 'import-type',
          canonicalOriginTarget: 'src/views/inner.ts::Thing',
        },
        {
          kind: 'dynamic-import',
          canonicalOriginTarget: 'src/views/outer.ts',
        },
        {
          kind: 'dynamic-import',
          canonicalOriginTarget: 'src/views/inner.ts',
        },
        {
          kind: 'export-assignment',
          canonicalOriginTarget: 'src/views/outer.ts',
        },
        {
          kind: 'dynamic-import',
          canonicalOriginTarget: 'src/views/inner.ts',
        },
      ]);
    });
  });

  it('rejects forged declarations outside the exact audit source root', () => {
    const isolatedPath = 'src/views/out-of-root.ts';
    const isolatedSource = "import { forged } from '../../sibling/src/forged'; void forged;";
    withIsolatedTypeScriptProject(
      new Map([
        [isolatedPath, isolatedSource],
        ['sibling/src/forged.ts', 'export const forged = true;'],
      ]),
      (context) => {
        let failure: unknown;
        try {
          resolvedModuleDependencies(context, { path: isolatedPath, source: isolatedSource });
        } catch (error) {
          failure = error;
        }
        expect(failure).toBeInstanceOf(ModuleDependencyResolutionError);
        expect(failure).toMatchObject({
          kind: 'unresolved-module-dependency',
          sourceFile: isolatedPath,
          writtenSpecifier: '../../sibling/src/forged',
        });
      },
    );

    const sandbox = mkdtempSync(join(tmpdir(), 'yssbi-typescript-source-root-'));
    const importerPath = join(sandbox, 'src', 'views', 'screen.ts');
    const forgedPath = join(sandbox, 'run-1', 'src', 'forged.ts');
    const configPath = join(sandbox, 'tsconfig.json');
    const source = "import { forged } from '../../run-1/src/forged'; void forged;";
    mkdirSync(join(sandbox, 'src', 'views'), { recursive: true });
    mkdirSync(join(sandbox, 'run-1', 'src'), { recursive: true });
    writeFileSync(importerPath, source);
    writeFileSync(forgedPath, 'export const forged = true;');
    writeFileSync(configPath, JSON.stringify({
      compilerOptions: { noLib: true, strict: true, target: 'esnext' },
      files: ['src/views/screen.ts', 'run-1/src/forged.ts'],
    }));

    let failure: unknown;
    try {
      withProductionTypeScriptProject((context) => {
        expect(productionTypeScriptSources(context).map(({ path }) => path)).toEqual([
          'src/views/screen.ts',
        ]);
        resolvedModuleDependencies(context, { path: importerPath, source });
      }, configPath);
    } catch (error) {
      failure = error;
    } finally {
      closeTypeScriptAuditResources();
      rmSync(sandbox, { recursive: true, force: true });
    }
    expect(failure).toBeInstanceOf(ModuleDependencyResolutionError);
    expect(failure).toMatchObject({
      kind: 'unresolved-module-dependency',
      sourceFile: importerPath,
      writtenSpecifier: '../../run-1/src/forged',
    });
  });

  it('fails closed for recognized dependencies without literal specifiers', () => {
    const cases = [
      ['src/views/nonliteral-import-type.ts', 'type Contract = import(Target).Contract;'],
      ['src/views/missing-dynamic-import-argument.ts', 'const loaded = import(); void loaded;'],
    ] as const;

    withIsolatedTypeScriptProject(new Map(cases), (context) => {
      for (const [path, source] of cases) {
        let failure: unknown;
        try {
          resolvedModuleDependencies(context, { path, source });
        } catch (error) {
          failure = error;
        }
        expect(failure, path).toBeInstanceOf(ModuleDependencyResolutionError);
        expect(failure, path).toMatchObject({
          kind: 'nonliteral-module-specifier',
          sourceFile: path,
          writtenSpecifier: null,
        });
      }
    });
  });

  it('inventories the complete frontend production tree', () => {
    const isolatedSources = new Map<string, string>([
      ['src/app/bootstrap.ts', 'export const bootstrap = true;'],
      ['src/views/screen.tsx', 'export const screen = null;'],
      ['src/features/application/useCase.ts', 'export const useCase = true;'],
      ['src/services/backend.ts', 'export const backend = true;'],
      ['src/components/control.tsx', 'export const control = null;'],
      ['src/shared/utils/kept.ts', 'export const kept = true;'],
      ['src/utils/diagnostic.ts', 'export const diagnostic = true;'],
      ['src/tests/helper.ts', 'export const testHelper = true;'],
      ['src/shared/behavior.test.ts', 'export const test = true;'],
      ['src/shared/namedFixture.ts', 'export const fixture = true;'],
      ['src/shared/generated.generated.ts', 'export const generated = true;'],
      ['src/shared/contracts.d.ts', 'export interface Contract {}'],
    ]);

    withIsolatedTypeScriptProject(isolatedSources, (context) => {
      expect(productionTypeScriptSources(context).map(({ path }) => path)).toEqual([
        'src/app/bootstrap.ts',
        'src/components/control.tsx',
        'src/features/application/useCase.ts',
        'src/services/backend.ts',
        'src/shared/utils/kept.ts',
        'src/utils/diagnostic.ts',
        'src/views/screen.tsx',
      ]);
      expect(productionTypeScriptSources(context).every(({ path }) => (
        path.startsWith('src/') && !path.includes('\\')
      ))).toBe(true);
    });

    withProductionTypeScriptProject((context) => {
      const productionPaths = productionTypeScriptSources(context).map(({ path }) => path);
      for (const root of [
        'src/app/',
        'src/views/',
        'src/features/',
        'src/services/',
        'src/components/',
        'src/shared/',
        'src/utils/',
      ]) {
        expect(productionPaths.some((path) => path.startsWith(root)), root).toBe(true);
      }
    });
  });

  it('fails closed for an unresolvable external stylesheet package', () => {
    const path = 'src/views/missingStylesheetPackage.ts';
    const source = "import 'not-a-real-package/theme.css';";
    let failure: unknown;

    withIsolatedTypeScriptProject(
      { [path]: source },
      (context) => {
        try {
          resolvedModuleDependencies(context, { path, source });
        } catch (error) {
          failure = error;
        }
      },
    );

    expect(failure).toBeInstanceOf(ModuleDependencyResolutionError);
    expect(failure).toMatchObject({
      kind: 'unresolved-module-dependency',
      sourceFile: path,
      writtenSpecifier: 'not-a-real-package/theme.css',
    });
  });

  it('emits declaration facts for empty named imports and exports', () => {
    const path = 'src/views/emptyNamedDeclarations.ts';
    const sources = new Map<string, string>([
      [path, `
        import {} from './runtime';
        import type {} from './contract';
        export {} from './runtime';
        export type {} from './contract';
      `],
      ['src/views/runtime.ts', 'export const value = 1;'],
      ['src/views/contract.ts', 'export interface Contract {}'],
    ]);

    withIsolatedTypeScriptProject(sources, (context) => {
      expect(resolvedModuleDependencies(context, { path, source: sources.get(path)! })).toEqual([
        expect.objectContaining({
          kind: 'static-import',
          mode: 'runtime',
          importedSymbol: null,
          canonicalOriginTarget: 'src/views/runtime.ts',
          symbolDeclarationTarget: 'src/views/runtime.ts',
        }),
        expect.objectContaining({
          kind: 'static-import',
          mode: 'type-only',
          importedSymbol: null,
          canonicalOriginTarget: 'src/views/contract.ts',
          symbolDeclarationTarget: 'src/views/contract.ts',
        }),
        expect.objectContaining({
          kind: 're-export',
          mode: 'runtime',
          importedSymbol: null,
          canonicalOriginTarget: 'src/views/runtime.ts',
          symbolDeclarationTarget: 'src/views/runtime.ts',
        }),
        expect.objectContaining({
          kind: 're-export',
          mode: 'type-only',
          importedSymbol: null,
          canonicalOriginTarget: 'src/views/contract.ts',
          symbolDeclarationTarget: 'src/views/contract.ts',
        }),
      ]);
    });
  });

  it('reports each invalid stylesheet input exactly without granting a dependency', () => {
    const path = 'src/views/fixture.css';
    const cases = [
      {
        name: 'unterminated quoted import',
        source: '@import "unterminated.css;',
        error: { kind: 'stylesheet-parse-failure', sourceFile: path, line: 1, column: 9 },
      },
      {
        name: 'nonliteral import',
        source: '@import url(var(--theme));',
        error: { kind: 'stylesheet-parse-failure', sourceFile: path, line: 1, column: 16 },
      },
      {
        name: 'remote import',
        source: '@import "https://example.invalid/theme.css";',
        error: {
          kind: 'unsupported-stylesheet-target',
          sourceFile: path,
          writtenSpecifier: 'https://example.invalid/theme.css',
        },
      },
      {
        name: 'repository escape',
        source: '@import "../../../outside.css";',
        error: {
          kind: 'stylesheet-path-escapes-repository',
          sourceFile: path,
          writtenSpecifier: '../../../outside.css',
        },
      },
      {
        name: 'unsupported parent stylesheet',
        source: '@import "../parent.css";',
        error: {
          kind: 'unsupported-stylesheet-target',
          sourceFile: path,
          writtenSpecifier: '../parent.css',
        },
      },
      {
        name: 'normalized parent stylesheet',
        source: '@import "./../parent.css";',
        error: {
          kind: 'unsupported-stylesheet-target',
          sourceFile: path,
          writtenSpecifier: './../parent.css',
        },
      },
      {
        name: 'encoded package separator',
        source: '@import "react/%2fsecret";',
        error: {
          kind: 'unsupported-stylesheet-target',
          sourceFile: path,
          writtenSpecifier: 'react/%2fsecret',
        },
      },
      {
        name: 'missing repository stylesheet',
        source: '@import "./missing.css";',
        error: {
          kind: 'stylesheet-target-missing',
          sourceFile: path,
          canonicalTarget: 'src/views/missing.css',
        },
      },
      {
        name: 'unsupported repository asset',
        source: '@import "./font.woff2";',
        error: {
          kind: 'unsupported-stylesheet-target',
          sourceFile: path,
          writtenSpecifier: './font.woff2',
        },
      },
      {
        name: 'quoted backslash',
        source: '@import "react\\secret";',
        error: {
          kind: 'unsupported-stylesheet-target',
          sourceFile: path,
          writtenSpecifier: 'react\\secret',
        },
      },
    ] as const;

    for (const fixture of cases) {
      const graph = resolvedStylesheetDependencies(
        resolve('.'),
        [stylesheetRoot(path)],
        new FixtureTextReader(new Map([
          [path, fixture.source],
          ['src/parent.css', '.parent {}'],
        ])),
      );
      expect(graph.dependencies, fixture.name).toEqual([]);
      expect(graph.errors, fixture.name).toEqual([fixture.error]);
    }
  });

  it('reads repository text only through real root-bounded src paths', () => {
    const sandbox = mkdtempSync(join(tmpdir(), 'yssbi-repository-reader-'));
    const repositoryRoot = join(sandbox, 'repository');
    const outsideRoot = join(sandbox, 'outside');
    const sourceRoot = join(repositoryRoot, 'src');
    mkdirSync(join(sourceRoot, 'app'), { recursive: true });
    mkdirSync(outsideRoot);
    writeFileSync(join(sourceRoot, 'app', 'App.css'), '.app {}');
    writeFileSync(join(outsideRoot, 'secret.css'), '.secret {}');
    symlinkSync(outsideRoot, join(sourceRoot, 'escape'), 'junction');

    try {
      const reader = createRepositoryTextReader(repositoryRoot);
      expect(reader.readRepositoryText('src/app/App.css')).toBe('.app {}');
      expect(reader.readRepositoryText(resolve(sourceRoot, 'app', 'App.css'))).toBeNull();
      expect(reader.readRepositoryText('package.json')).toBeNull();
      expect(reader.readRepositoryText('../outside/secret.css')).toBeNull();
      expect(reader.readRepositoryText('src/escape/secret.css')).toBeNull();
    } finally {
      rmSync(sandbox, { recursive: true, force: true });
    }
  });

  it('builds the real App and workbench stylesheet graph through the repository reader', () => {
    withProductionTypeScriptProject((context) => {
      const sources = new Map(productionTypeScriptSources(context).map((source) => [source.path, source]));
      const moduleDependencies = [
        'src/app/App.tsx',
        'src/app/main.tsx',
      ].flatMap((path) => resolvedModuleDependencies(context, sources.get(path)!));
      const graph = resolvedStylesheetDependencies(
        resolve('.'),
        moduleDependencies,
        createRepositoryTextReader(resolve('.')),
      );

      expect(graph.repositoryStylesheets).toEqual([
        'src/app/App.css',
        'src/app/workbench-dockview.css',
      ]);
      expect(graph.dependencies.map(({ canonicalOriginTarget }) => canonicalOriginTarget)).toEqual([
        'external:tailwindcss',
        'external:tw-animate-css',
        'external:shadcn::tailwind.css',
        'external:@fontsource-variable/inter',
      ]);
      expect(graph.errors).toEqual([]);
    });
  });
});
