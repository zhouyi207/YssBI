import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
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
  type ResolvedModuleDependency,
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
