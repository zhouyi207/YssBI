import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { ArchitectureSource } from '@/tests/helpers/moduleDependencyAudit';
import {
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
} from '@/tests/helpers/typescriptAudit';
import {
  ModuleDependencyResolutionError,
  productionTypeScriptSources,
  resolvedModuleDependencies,
  resolvedStylesheetDependencies,
  type RepositoryTextReader,
} from './frontendArchitectureModel';

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

describe('frontend architecture model', () => {
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
});
