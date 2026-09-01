import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ArchitectureSource } from "@/tests/helpers/moduleDependencyAudit";
import {
  closeTypeScriptAuditResources,
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
} from "@/tests/helpers/typescriptAudit";
import {
  ModuleDependencyResolutionError,
  createRepositoryTextReader,
  productionTypeScriptSources,
  resolvedModuleDependencies,
  resolvedStylesheetDependencies,
  type RepositoryTextReader,
  type AssetDependencyPolicy,
  type ExternalDependencyPolicy,
  type ReadonlyPackageManifest,
  type ResolvedModuleDependency,
  type ResolvedStylesheetDependency,
  type ResolvedStylesheetGraph,
} from "./frontendArchitectureModel";
import {
  classifyFrontendSources,
  type FrontendBaseRule,
  type FrontendLayer,
  type FrontendLiteralPolicyMembership,
} from "./frontendArchitecturePolicy";
import {
  FRONTEND_ASSET_DEPENDENCY_POLICY,
  auditFrontendAssetDependencies,
} from "./frontendAssetDependencyPolicy";
import {
  FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
  auditFrontendExternalDependencies,
} from "./frontendExternalDependencyPolicy";
import { FRONTEND_ARCHITECTURE_POLICY } from "./frontendArchitecturePolicy";

class FixtureTextReader implements RepositoryTextReader {
  constructor(private readonly files: ReadonlyMap<string, string>) {}

  readRepositoryText(repositoryRelativePath: string): string | null {
    return this.files.get(repositoryRelativePath) ?? null;
  }
}

const compilerSources = new Map<string, string>([
  [
    "src/views/fixture.tsx",
    `
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
  `,
  ],
  ["src/views/export-assignment.ts", "export = require('./runtime');"],
  ["src/views/runtime.ts", "export const runtimeValue = 1;"],
  ["src/features/core/fixture/read.ts", "export const approvedRead = 1;"],
  [
    "src/features/domain/fixture/contract.ts",
    "export interface Contract { readonly value: string; }",
  ],
  ["src/features/domain/fixture/contract-barrel.ts", "export { Contract } from './contract';"],
  [
    "src/features/domain/fixture/aliased-barrel.ts",
    "export { Contract as RenamedContract } from './contract-barrel';",
  ],
  [
    "src/features/domain/fixture/mixed.ts",
    `
    export const runtimeValue = 1;
    export interface MixedContract { readonly value: number; }
  `,
  ],
  [
    "src/app/i18n-facade.ts",
    `
    import i18n from 'i18next';
    export { i18n };
  `,
  ],
  [
    "src/features/application/i18n-user.ts",
    `
    import { i18n } from '../../app/i18n-facade';
    void i18n;
  `,
  ],
  [
    "node_modules/@types/react/index.d.ts",
    `
    export interface ReactNode { readonly reactNode: unique symbol; }
    export interface ReactTypesOnly { readonly forbiddenReact: unique symbol; }
  `,
  ],
  [
    "node_modules/@types/d3/index.d.ts",
    `
    export interface Selection { readonly selection: unique symbol; }
    export interface D3TypesOnly { readonly forbiddenD3: unique symbol; }
  `,
  ],
  ["node_modules/react-dom/client.d.ts", "export declare function createRoot(): void;"],
  ["node_modules/@tauri-apps/api/window.d.ts", "export declare function getCurrentWindow(): void;"],
  [
    "node_modules/i18next/index.d.ts",
    "declare const i18n: { readonly language: string }; export default i18n;",
  ],
  ["src/globals.d.ts", "declare function require(specifier: string): unknown;"],
]);

const stylesheetSources = new Map<string, string>([
  [
    "src/views/fixture.css",
    `
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
  `,
  ],
  ["src/views/nested.css", ".nested { color: red; }"],
  ["src/views/url-target.css", ".url-target { color: blue; }"],
  ["src/views/cycle-a.css", '@import "./cycle-b.css";'],
  ["src/views/cycle-b.css", '@import "./cycle-a.css";'],
  ["src/views/malformed.css", '@import "unterminated.css;'],
]);

function architectureSource(path: string): ArchitectureSource {
  const source = compilerSources.get(path);
  if (source === undefined) throw new Error(`Missing compiler fixture ${path}`);
  return { path, source };
}

function stylesheetRoot(path: string): ResolvedModuleDependency {
  const fileName = path.split("/").slice(-1)[0];
  return {
    kind: "side-effect-import",
    mode: "runtime",
    specifier: `./${fileName}`,
    location: { line: 1, column: 1 },
    repositoryRelativeSourceFile: "src/views/fixture.ts",
    fullyQualifiedOwner: "src/views/fixture.ts::<module>",
    origin: {
      kind: "repository-asset",
      asset: { repositoryRelativeAssetPath: path, resourceKind: "stylesheet" },
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
  mode: "runtime" | "type-only",
  resourceKind: "module" | "stylesheet" = "module",
  symbolDeclarationTarget: string | null = null,
): ResolvedModuleDependency {
  const writtenSubpath = canonicalSubpath?.split("::").join("/");
  const writtenModuleSpecifier = `${packageName}${writtenSubpath ? `/${writtenSubpath}` : ""}`;
  return {
    kind: resourceKind === "stylesheet" ? "side-effect-import" : "static-import",
    mode,
    specifier: writtenModuleSpecifier,
    location: { line: 1, column: 1 },
    repositoryRelativeSourceFile: sourceFile,
    fullyQualifiedOwner: `${sourceFile}::<module>`,
    origin: {
      kind: "external",
      dependency: { packageName, canonicalSubpath, resourceKind },
    },
    canonicalOriginTarget: `external:${packageName}${canonicalSubpath ? `::${canonicalSubpath}` : ""}`,
    importedSymbol: resourceKind === "module" ? "fixture" : null,
    writtenModuleSpecifier,
    symbolDeclarationTarget,
  };
}

function repositoryAssetDependency(
  sourceFile: string,
  assetPath: string,
): ResolvedModuleDependency {
  const assetFileName = assetPath.split("/").slice(-1)[0];
  return {
    kind: "side-effect-import",
    mode: "runtime",
    specifier: `./${assetFileName}`,
    location: { line: 1, column: 1 },
    repositoryRelativeSourceFile: sourceFile,
    fullyQualifiedOwner: `${sourceFile}::<module>`,
    origin: {
      kind: "repository-asset",
      asset: { repositoryRelativeAssetPath: assetPath, resourceKind: "stylesheet" },
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
  const writtenSubpath = canonicalSubpath?.split("::").join("/");
  const writtenSpecifier = `${packageName}${writtenSubpath ? `/${writtenSubpath}` : ""}`;
  return {
    repositoryRelativeSourceFile: sourceFile,
    fullyQualifiedOwner: `stylesheet:${sourceFile}`,
    kind: "stylesheet-import",
    mode: "build-style",
    origin: {
      kind: "external",
      dependency: { packageName, canonicalSubpath, resourceKind: "stylesheet" },
    },
    canonicalOriginTarget: `external:${packageName}${canonicalSubpath ? `::${canonicalSubpath}` : ""}`,
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
    kind: "stylesheet-import",
    mode: "build-style",
    origin: {
      kind: "repository-asset",
      asset: { repositoryRelativeAssetPath: assetPath, resourceKind: "stylesheet" },
    },
    canonicalOriginTarget: `repository-asset:${assetPath}`,
    writtenSpecifier: `./${assetPath.split("/").slice(-1)[0]}`,
    line: 1,
    column: 1,
  };
}

describe("frontend architecture model", () => {
  it("keeps Dockview as the sole editor panel topology authority", () => {
    withProductionTypeScriptProject((context) => {
      const sources = productionTypeScriptSources(context);
      const obsoleteIdentifiers = [
        "LayoutTab",
        "LayoutTabType",
        "LayoutTabComponent",
        "EditorGroupSnapshot",
        "layoutTabFromEditorMetadata",
        "useTabManagement",
      ];
      const violations = sources.flatMap(({ path, source }) =>
        obsoleteIdentifiers
          .filter((identifier) => new RegExp(`\\b${identifier}\\b`, "u").test(source))
          .map((identifier) => `${path}:${identifier}`),
      );
      expect(violations).toEqual([]);

      const obsoleteFiles = new Set([
        "src/features/core/layout/layoutTabModel.ts",
        "src/features/core/layout/layoutTabQueries.ts",
        "src/features/application/editor/dockviewTabProjection.ts",
      ]);
      expect(sources.map(({ path }) => path).filter((path) => obsoleteFiles.has(path))).toEqual([]);
      expect(
        sources
          .map(({ path }) => path)
          .filter(
            (path) =>
              path.startsWith("src/features/application/layout/") ||
              path.startsWith("src/features/core/layout/"),
          ),
      ).toEqual([]);

      const layoutConsumers = sources.filter(
        ({ path, source }) =>
          path !== "src/modules/workbench/internal/application/useWorkbenchLayout.ts" &&
          /\buseWorkbenchLayout\s*\(/u.test(source),
      );
      expect(layoutConsumers.map(({ path }) => path)).toEqual([
        "src/modules/workbench/internal/dockview/RootDockviewHost.tsx",
      ]);

      const obsoleteShellFiles = new Set([
        "src/views/EditorView/EditorWindow.tsx",
        "src/views/EditorView/Layout/Workspace.tsx",
        "src/views/EditorView/Layout/WorkbenchDockviewPanels.tsx",
      ]);
      expect(
        sources.map(({ path }) => path).filter((path) => obsoleteShellFiles.has(path)),
      ).toEqual([]);

      const legacyEditorComponentIds = sources.flatMap(({ path, source }) =>
        [...source.matchAll(/["'](?:GraphEditor|ChartEditor)["']/gu)].map(
          ([componentId]) => `${path}:${componentId}`,
        ),
      );
      expect(legacyEditorComponentIds).toEqual([]);

      const rootPanelRegistryOwners = sources
        .filter(({ source }) => /\brootPanelRegistry\b/u.test(source))
        .map(({ path }) => path)
        .sort();
      expect(rootPanelRegistryOwners).toEqual([
        "src/app/windows/workbench/WorkbenchComposition.tsx",
        "src/app/windows/workbench/rootPanelRegistry.tsx",
      ]);

      const concreteEditorRegistryOwners = sources.filter(
        ({ source }) => /\bGraphDocumentEditor\b/u.test(source) && /\bChartEditor\b/u.test(source),
      );
      expect(concreteEditorRegistryOwners.map(({ path }) => path)).toEqual([
        "src/app/windows/workbench/editorRendererRegistry.ts",
      ]);
    });
  });

  it("keeps business coordination outside the root Dockview adapters", () => {
    withProductionTypeScriptProject((context) => {
      const sourceByPath = new Map(
        productionTypeScriptSources(context).map(({ path, source }) => [path, source]),
      );
      const rootHost =
        sourceByPath.get("src/modules/workbench/internal/dockview/RootDockviewHost.tsx") ?? "";
      const tabRenderer =
        sourceByPath.get("src/modules/workbench/internal/dockview/RootPanelTabRenderer.tsx") ?? "";
      const dragOverlay =
        sourceByPath.get("src/modules/workbench/internal/ui/dnd/SidebarDragOverlay.tsx") ?? "";
      const layoutActions =
        sourceByPath.get("src/modules/workbench/internal/application/workbenchLayoutActions.ts") ??
        "";
      const directBusinessDockviewConsumers = [...sourceByPath.entries()]
        .filter(
          ([path, source]) =>
            path.startsWith("src/modules/") &&
            !path.startsWith("src/modules/workbench/") &&
            /from\s+["']dockview-react["']/u.test(source),
        )
        .map(([path]) => path)
        .sort();

      expect(rootHost).not.toMatch(/\b(?:synchronizeActiveEditorPanel|useSettingsRead)\b/u);
      expect(tabRenderer).not.toMatch(/from\s+["']@\/features\//u);
      expect(dragOverlay).not.toMatch(/from\s+["']@\/features\//u);
      expect(layoutActions).not.toMatch(/\b(?:useEditorStore|useGraphSessionStore)\b/u);
      expect(directBusinessDockviewConsumers).toEqual([
        "src/modules/logs/internal/ui/LogDomainDockviewHost.tsx",
        "src/modules/logs/internal/ui/LogDomainPanel.tsx",
        "src/modules/logs/internal/ui/LogWorkspaceActions.tsx",
      ]);

      expect(
        sourceByPath.get("src/app/windows/workbench/integrations/panelActivationCoordinator.ts"),
      ).toMatch(/\bsynchronizeActiveEditorPanel\b/u);
      expect(sourceByPath.get("src/app/windows/workbench/rootPanelTabRenderer.tsx")).toMatch(
        /\brequestCloseEditorPanel\b/u,
      );
      expect(
        sourceByPath.get("src/app/windows/workbench/integrations/activityEditorDndOverlay.tsx"),
      ).toMatch(/\buseActivityEditorDragOverlayLabel\b/u);
    });
  });

  it("keeps Workbench chrome prop-driven and app-composed", () => {
    withProductionTypeScriptProject((context) => {
      const sources = productionTypeScriptSources(context);
      const sourceByPath = new Map(sources.map(({ path, source }) => [path, source]));
      const pureChromeFiles = [
        "src/modules/workbench/internal/ui/WorkbenchWindow.tsx",
        "src/modules/workbench/internal/ui/menu/WorkbenchMenuBar.tsx",
        "src/modules/workbench/internal/ui/menu/AboutModal.tsx",
        "src/modules/workbench/internal/ui/status/StatusBar.tsx",
        "src/modules/workbench/internal/ui/status/StatusBarItem.tsx",
      ];
      for (const path of pureChromeFiles) {
        expect(sourceByPath.get(path) ?? "", path).not.toMatch(/from\s+["']@\/features\//u);
      }
      for (const { path, source } of sources.filter(({ path }) =>
        path.startsWith("src/modules/workbench/internal/ui/"),
      )) {
        expect(source, path).not.toMatch(/from\s+["']@\/features\//u);
      }

      const composition =
        sourceByPath.get("src/app/windows/workbench/WorkbenchComposition.tsx") ?? "";
      expect(composition).toMatch(/\buseAppInitialization\b/u);
      expect(composition).toMatch(/\buseProjectSync\b/u);
      expect(composition).toMatch(/\buseEditorKeyboard\b/u);
      expect(
        sourceByPath.get("src/app/windows/workbench/menuContributionRegistry.tsx") ?? "",
      ).toMatch(/\buseMenubar\b/u);
      expect(
        sourceByPath.get("src/app/windows/workbench/statusBarContributionRegistry.tsx") ?? "",
      ).toMatch(/\buseStatusBarItems\b/u);
    });
  });

  it("keeps the chart resource cutover single-path", () => {
    withProductionTypeScriptProject((context) => {
      const retiredResourceTerm = ["work", "sheet"].join("");
      const leftovers = productionTypeScriptSources(context).flatMap(({ path, source }) => {
        const findings: string[] = [];
        if (path.toLowerCase().includes(retiredResourceTerm)) findings.push(`${path}:path`);
        if (source.toLowerCase().includes(retiredResourceTerm)) findings.push(`${path}:source`);
        return findings;
      });

      expect(leftovers).toEqual([]);

      const chartEditor = productionTypeScriptSources(context).find(
        ({ path }) => path === "src/modules/chart/internal/ui/ChartEditor.tsx",
      )?.source;
      expect(chartEditor).toContain("resourceRef");
      expect(chartEditor).not.toMatch(/\b(?:GroupContext|activeTabId|useEditorGroupWorkspace)\b/u);
    });
  });

  it("keeps module internals private behind root public APIs", () => {
    withProductionTypeScriptProject((context) => {
      const sources = productionTypeScriptSources(context);
      const paths = new Set(sources.map(({ path }) => path));
      const moduleNames = new Set<string>();
      const deepImports: string[] = [];
      const moduleDependencies = new Map<string, Set<string>>();
      const directBusinessImports: string[] = [];

      for (const { path, source } of sources) {
        const ownerMatch = /^src\/modules\/([^/]+)\//u.exec(path);
        const ownerModule = ownerMatch?.[1] ?? null;
        if (ownerModule) {
          moduleNames.add(ownerModule);
          moduleDependencies.set(ownerModule, moduleDependencies.get(ownerModule) ?? new Set());
        }

        for (const match of source.matchAll(/["']@\/modules\/([^/"']+)\/([^"']+)["']/gu)) {
          const [, moduleName, subpath] = match;
          if (subpath === "public" || path.startsWith(`src/modules/${moduleName}/`)) continue;
          deepImports.push(`${path}:@/modules/${moduleName}/${subpath}`);
        }

        if (!ownerModule) continue;
        for (const match of source.matchAll(/["']@\/modules\/([^/"']+)\/public["']/gu)) {
          const targetModule = match[1];
          if (targetModule === ownerModule) continue;
          moduleDependencies.get(ownerModule)?.add(targetModule);
          if (targetModule !== "workbench") {
            directBusinessImports.push(`${path}:${ownerModule}->${targetModule}`);
          }
        }
      }

      expect(deepImports).toEqual([]);
      expect(directBusinessImports).toEqual([]);
      expect(
        [...moduleNames]
          .map((moduleName) => `src/modules/${moduleName}/public.ts`)
          .filter((path) => !paths.has(path))
          .sort(),
      ).toEqual([]);

      const cyclicModules = [...moduleNames].filter((startModule) => {
        const pending = [...(moduleDependencies.get(startModule) ?? [])];
        const visited = new Set<string>();
        while (pending.length > 0) {
          const current = pending.pop();
          if (!current || visited.has(current)) continue;
          if (current === startModule) return true;
          visited.add(current);
          pending.push(...(moduleDependencies.get(current) ?? []));
        }
        return false;
      });
      expect(cyclicModules.sort()).toEqual([]);
    });
  });

  it("keeps final workbench and panel vocabulary", () => {
    withProductionTypeScriptProject((context) => {
      const sources = productionTypeScriptSources(context);
      const retiredIdentifiers = [
        "BottomBar",
        "GraphEditor",
        "LogWorkspaceDockview",
        "WorkbenchDockviewTab",
        "WorkbenchStore",
        "activeTabId",
      ];
      expect(
        sources.flatMap(({ path, source }) =>
          retiredIdentifiers
            .filter((identifier) => new RegExp(`\\b${identifier}\\b`, "u").test(source))
            .map((identifier) => `${path}:${identifier}`),
        ),
      ).toEqual([]);

      const paths = new Set(sources.map(({ path }) => path));
      const retiredFiles = [
        "src/features/core/workbench/workbenchStore.ts",
        "src/modules/graph-editor/internal/ui/Canvas/core/GraphEditor.tsx",
        "src/views/EditorView/Layout/BottomBar.tsx",
        "src/views/EditorView/Layout/WorkbenchDockviewTab.tsx",
        "src/modules/logs/internal/ui/LogWorkspaceDockview.tsx",
      ];
      expect(retiredFiles.filter((path) => paths.has(path))).toEqual([]);

      const finalFiles = [
        "src/modules/workbench/internal/state/workbenchUiStore.ts",
        "src/modules/graph-editor/internal/ui/Canvas/core/GraphDocumentEditor.tsx",
        "src/modules/workbench/internal/dockview/RootPanelTabRenderer.tsx",
        "src/modules/workbench/internal/ui/status/StatusBar.tsx",
        "src/modules/logs/internal/ui/LogDomainDockviewHost.tsx",
      ];
      expect(finalFiles.filter((path) => !paths.has(path))).toEqual([]);
    });
  });

  it("keeps shared packages limited to reusable contracts and presentation", () => {
    withProductionTypeScriptProject((context) => {
      const paths = productionTypeScriptSources(context).map(({ path }) => path);
      const pathSet = new Set(paths);
      const retiredPrefixes = [
        "src/shared/types/state/",
        "src/shared/types/store/",
        "src/shared/types/visualization/",
      ];
      const retiredFiles = new Set([
        "src/shared/types/dto/graphCommands.ts",
        "src/shared/types/dto/graphConverters.ts",
        "src/shared/types/dto/graphModel.ts",
        "src/shared/types/dto/pinHydrate.ts",
        "src/shared/types/ui/application.ts",
        "src/shared/types/ui/detail.ts",
        "src/shared/types/ui/editor.ts",
        "src/shared/types/ui/execution.ts",
        "src/shared/ui/ExcelSheetSelectModal.tsx",
        "src/shared/ui/ImportModal.tsx",
        "src/shared/ui/SqlConnectionModal.tsx",
        "src/shared/ui/SqliteTableSelectModal.tsx",
        "src/shared/ui/SqlRemoteTableSelectModal.tsx",
        "src/shared/utils/pinCompatibility.ts",
      ]);

      expect(
        paths.filter(
          (path) =>
            retiredPrefixes.some((prefix) => path.startsWith(prefix)) || retiredFiles.has(path),
        ),
      ).toEqual([]);

      const ownerFiles = [
        "src/features/core/dataStore/nodeView.ts",
        "src/features/core/editor/detail/detailTypes.ts",
        "src/features/core/execution/executionTypes.ts",
        "src/features/core/ui/applicationUiTypes.ts",
        "src/features/domain/editorProjection/connectionRules.ts",
        "src/features/domain/editorProjection/graphRuntimeTypes.ts",
        "src/shared/charts/ChartModel.ts",
        "src/modules/data-explorer/internal/ui/import/ImportModal.tsx",
      ];
      expect(ownerFiles.filter((path) => !pathSet.has(path))).toEqual([]);
    });
  });

  it("keeps Activity panels independently contributed and DnD composed by the app", () => {
    withProductionTypeScriptProject((context) => {
      const sources = productionTypeScriptSources(context);
      const retiredActivityFiles = new Set([
        "src/views/EditorView/Layout/WorkbenchActivityPanels.tsx",
        "src/views/EditorView/Layout/sidebar/useSidebarResourceActions.ts",
      ]);
      expect(
        sources.map(({ path }) => path).filter((path) => retiredActivityFiles.has(path)),
      ).toEqual([]);

      const retiredIdentifiers = [
        "WorkbenchActivityPanelsProvider",
        "WorkbenchActivityPanelsContext",
        "useSidebarResourceActions",
      ];
      expect(
        sources.flatMap(({ path, source }) =>
          retiredIdentifiers
            .filter((identifier) => new RegExp(`\\b${identifier}\\b`, "u").test(source))
            .map((identifier) => `${path}:${identifier}`),
        ),
      ).toEqual([]);

      const contributions = [
        "projectActivityPanelContribution",
        "nodeCatalogActivityPanelContribution",
        "dataActivityPanelContribution",
        "commandsActivityPanelContribution",
      ];
      const multiPanelConsumers = sources.filter(
        ({ source }) =>
          contributions.filter((contribution) =>
            new RegExp(`\\b${contribution}\\b`, "u").test(source),
          ).length > 1,
      );
      expect(multiPanelConsumers.map(({ path }) => path)).toEqual([
        "src/app/windows/workbench/rootPanelRegistry.tsx",
      ]);

      const rootDockviewHost = sources.find(
        ({ path }) => path === "src/modules/workbench/internal/dockview/RootDockviewHost.tsx",
      )?.source;
      expect(rootDockviewHost).not.toMatch(
        /\b(?:beginActivityEditorDrag|executeEditorDragEnd|sidebarDragUi)\b|["']pointermove["']/u,
      );

      const dndCoordinator = sources.find(
        ({ path }) =>
          path === "src/app/windows/workbench/integrations/activityEditorDndCoordinator.ts",
      )?.source;
      expect(dndCoordinator).toMatch(/\bexecuteEditorDragEnd\b/u);
      expect(dndCoordinator).toMatch(/["']pointermove["']/u);
    });
  });

  it("keeps Workbench commands caller-shaped instead of globally aggregated", () => {
    withProductionTypeScriptProject((context) => {
      const sources = productionTypeScriptSources(context);
      const retiredEditorFacadeFiles = [
        "src/features/application/editor/EditorSessionContext.tsx",
        "src/features/application/editor/editorSessionCommands.ts",
        "src/features/application/editor/editorSessionTypes.ts",
        "src/features/application/editor/useEditorSessionCommands.ts",
        "src/features/application/editor/useEditorSessionSlices.ts",
        "src/features/application/editor/useEditorSessionUi.ts",
        "src/features/core/editor/hooks/useActiveEditorGroup.ts",
        "src/features/core/editor/hooks/useEditorActions.ts",
        "src/features/core/editor/hooks/useEditorCanvasActions.ts",
        "src/features/core/editor/hooks/useEditorGroupPlacement.ts",
        "src/features/core/editor/hooks/useEditorGroups.ts",
        "src/features/core/editor/hooks/useEditorUIState.ts",
        "src/features/core/editor/editorGroupSelection.ts",
        "src/features/core/editor/detail/variablesGraphScope.ts",
      ];
      expect(
        sources.map(({ path }) => path).filter((path) => retiredEditorFacadeFiles.includes(path)),
      ).toEqual([]);

      const retiredIdentifiers = [
        "EditorSessionCommands",
        "EditorSessionProvider",
        "useEditorSessionCommandsContext",
        "useEditorSessionResources",
        "useEditorSessionDetailActions",
      ];
      expect(
        sources.flatMap(({ path, source }) =>
          retiredIdentifiers
            .filter((identifier) => new RegExp(`\\b${identifier}\\b`, "u").test(source))
            .map((identifier) => `${path}:${identifier}`),
        ),
      ).toEqual([]);

      const coordinatorConsumers = sources.filter(
        ({ path, source }) =>
          path !== "src/app/windows/workbench/integrations/workbenchCommandCoordinator.ts" &&
          /\buseWorkbenchCommandCoordinator\s*\(/u.test(source),
      );
      expect(coordinatorConsumers.map(({ path }) => path)).toEqual([
        "src/app/windows/workbench/WorkbenchComposition.tsx",
      ]);

      const coordinator = sources.find(
        ({ path }) =>
          path === "src/app/windows/workbench/integrations/workbenchCommandCoordinator.ts",
      )?.source;
      expect(coordinator).toMatch(/\buseEditorOperations\s*\(/u);
      expect(coordinator).toMatch(/\buseGraphManagement\s*\(/u);
      expect(coordinator).toMatch(/\buseChartManagement\s*\(/u);
    });
  });

  it("keeps Graph UI stateful controllers separate from prop-only views", () => {
    withProductionTypeScriptProject((context) => {
      const sources = productionTypeScriptSources(context);
      const sourceByPath = new Map(sources.map(({ path, source }) => [path, source]));
      const retiredGraphUiFiles = new Set([
        "src/features/application/editor/CanvasContextMenuContext.tsx",
        "src/modules/graph-editor/internal/ui/Canvas/core/Canvas.tsx",
        "src/modules/graph-editor/internal/ui/Nodes/CanvasNode.tsx",
        "src/modules/graph-editor/internal/ui/Nodes/Node.tsx",
        "src/modules/graph-editor/internal/ui/Nodes/NodeContainer.tsx",
        "src/modules/graph-editor/internal/ui/Pins/Pin.tsx",
      ]);
      expect(
        sources.map(({ path }) => path).filter((path) => retiredGraphUiFiles.has(path)),
      ).toEqual([]);

      const requiredGraphUiFiles = [
        "src/modules/graph-editor/internal/ui/Canvas/core/GraphCanvasController.tsx",
        "src/modules/graph-editor/internal/ui/Canvas/core/GraphCanvasView.tsx",
        "src/modules/graph-editor/internal/ui/Nodes/GraphNodeController.tsx",
        "src/modules/graph-editor/internal/ui/Nodes/GraphNodeView.tsx",
        "src/modules/graph-editor/internal/ui/Pins/GraphPinController.tsx",
        "src/modules/graph-editor/internal/ui/Pins/GraphPinView.tsx",
      ];
      expect(requiredGraphUiFiles.filter((path) => !sourceByPath.has(path))).toEqual([]);

      const retiredContextIdentifiers = [
        "CanvasContextMenuProvider",
        "useCanvasContextMenuActions",
        "useCanvasContextMenuActionsOptional",
      ];
      expect(
        sources.flatMap(({ path, source }) =>
          retiredContextIdentifiers
            .filter((identifier) => new RegExp(`\\b${identifier}\\b`, "u").test(source))
            .map((identifier) => `${path}:${identifier}`),
        ),
      ).toEqual([]);

      const viewContracts = [
        {
          path: "src/modules/graph-editor/internal/ui/Canvas/core/GraphCanvasView.tsx",
          slots: ["viewportGridSlot", "connectionPreviewSlot", "graphContentSlot", "overlaySlot"],
        },
        {
          path: "src/modules/graph-editor/internal/ui/Nodes/GraphNodeView.tsx",
          slots: ["contentSlot", "executionBadgeSlot", "diagnosticBadgeSlot", "contextMenuSlot"],
        },
        {
          path: "src/modules/graph-editor/internal/ui/Pins/GraphPinView.tsx",
          slots: ["inputSlot", "contextMenuSlot"],
        },
      ];
      for (const { path, slots } of viewContracts) {
        const source = sourceByPath.get(path) ?? "";
        expect(source, path).not.toMatch(/from\s+["']@\/(?:features|services)\//u);
        expect(source, path).not.toMatch(/\buse[A-Z]\w*\s*\(/u);
        for (const slot of slots) expect(source, `${path}:${slot}`).toContain(slot);
      }

      const watermarkView =
        sourceByPath.get(
          "src/modules/graph-editor/internal/ui/Canvas/overlays/WatermarkView.tsx",
        ) ?? "";
      expect(watermarkView).not.toMatch(/from\s+["']@\/features\/application\//u);
      expect(watermarkView).toContain("interface WatermarkCommands");

      const canvasController =
        sourceByPath.get(
          "src/modules/graph-editor/internal/ui/Canvas/core/GraphCanvasController.tsx",
        ) ?? "";
      expect(canvasController).toContain(
        "contextMenuActions={interactive ? contextMenuActions : null}",
      );
      expect(canvasController).toContain("overlaySlot=");
      expect(
        sourceByPath.get("src/modules/graph-editor/internal/ui/Nodes/GraphNodeController.tsx"),
      ).toContain("executionBadgeSlot=");
      expect(
        sourceByPath.get("src/modules/graph-editor/internal/ui/Pins/GraphPinController.tsx"),
      ).toContain("contextMenuSlot=");
    });
  });

  it("classifies every frontend production source exactly once", () => {
    const emptyMembership = Object.fromEntries(
      [
        "app-composition",
        "views",
        "application",
        "core",
        "domain",
        "services",
        "components-ui",
        "wire-schema",
        "diagnostics",
        "pure-shared",
      ].map((layer) => [layer, []]),
    ) as unknown as Record<FrontendLayer, readonly string[]>;
    const literalMembership: FrontendLiteralPolicyMembership = {
      ...emptyMembership,
      services: ["src/shared/platform/testAdapter.ts"],
      core: ["src/shared/overlap.ts"],
      diagnostics: ["src/shared/overlap.ts"],
    };
    const report = classifyFrontendSources(
      [
        { path: "src\\app\\fixture.ts", source: "export const fixture = true;" },
        { path: "src/modules/chart/public.ts", source: "export const chart = true;" },
        {
          path: "src/modules/chart/internal/ui/ChartEditor.tsx",
          source: "export const editor = true;",
        },
        {
          path: "src/modules/chart/internal/application/chartCommands.ts",
          source: "export const commands = true;",
        },
        {
          path: "src/modules/chart/internal/state/chartStore.ts",
          source: "export const store = true;",
        },
        {
          path: "src/modules/chart/internal/domain/ChartDocument.ts",
          source: "export const document = true;",
        },
        {
          path: "src/modules/workbench/internal/dockview/workbenchRead.ts",
          source: "export const read = true;",
        },
        {
          path: "src/modules/workbench/internal/dockview/RootDockviewHost.tsx",
          source: "export const host = true;",
        },
        { path: "src/shared/platform/testAdapter.ts", source: "export const adapter = true;" },
        { path: "src/unowned/fixture.ts", source: "export const fixture = true;" },
        { path: "src/shared/overlap.ts", source: "export const overlap = true;" },
      ],
      literalMembership,
    );

    expect([...report.classification]).toEqual([
      ["src/app/fixture.ts", "app-composition"],
      ["src/modules/chart/internal/application/chartCommands.ts", "application"],
      ["src/modules/chart/internal/domain/ChartDocument.ts", "domain"],
      ["src/modules/chart/internal/state/chartStore.ts", "core"],
      ["src/modules/chart/internal/ui/ChartEditor.tsx", "views"],
      ["src/modules/chart/public.ts", "views"],
      ["src/modules/workbench/internal/dockview/RootDockviewHost.tsx", "views"],
      ["src/modules/workbench/internal/dockview/workbenchRead.ts", "core"],
      ["src/shared/platform/testAdapter.ts", "services"],
    ]);
    expect(report.errors).toEqual([
      {
        kind: "multiply-classified-production-source",
        sourceFile: "src/shared/overlap.ts",
        layers: ["core", "diagnostics"],
      },
      {
        kind: "unclassified-production-source",
        sourceFile: "src/unowned/fixture.ts",
      },
    ]);

    withProductionTypeScriptProject((context) => {
      const productionReport = classifyFrontendSources(productionTypeScriptSources(context));
      expect(productionReport.errors).toEqual([]);
      expect(productionReport.classification.size).toBe(
        productionTypeScriptSources(context).length,
      );
    });
    expect(FRONTEND_ARCHITECTURE_POLICY.capabilities).toContainEqual(
      expect.objectContaining({
        canonicalModule: "src/modules/workbench/internal/dockview/workbenchRead.ts",
        exportedSymbols: ["WorkbenchDockviewRead"],
        memberCapabilities: {
          WorkbenchDockviewRead: [
            "isReady",
            "isHydrated",
            "whenHydrated",
            "subscribe",
            "getSnapshot",
            "getPanel",
            "getActivePanel",
            "getActiveEditorPanel",
            "getActiveEditorPanelInGroup",
            "listPanels",
            "listGroups",
            "listGroupPanels",
            "listEditorPanelsInGroup",
            "findEditorPanelsByResource",
            "getEdgeState",
          ],
        },
      }),
    );
    expect(
      FRONTEND_ARCHITECTURE_POLICY.capabilities
        .filter(
          (capability) =>
            capability.canonicalModule ===
              "src/modules/workbench/internal/dockview/workbenchRead.ts" &&
            capability.exportedSymbols.some(
              (symbol) => symbol === "WorkbenchDockviewRead" || symbol === "workbenchDockviewRead",
            ),
        )
        .map(({ sourceLayer }) => sourceLayer),
    ).toEqual(["app-composition", "views"]);
  });

  it("reports overlapping frontend base memberships without rule ordering", () => {
    const emptyMembership = Object.fromEntries(
      [
        "app-composition",
        "views",
        "application",
        "core",
        "domain",
        "services",
        "components-ui",
        "wire-schema",
        "diagnostics",
        "pure-shared",
      ].map((layer) => [layer, []]),
    ) as unknown as Record<FrontendLayer, readonly string[]>;
    const baseRules: readonly FrontendBaseRule[] = [
      { layer: "views", matches: (path) => path.endsWith("/base-overlap.ts") },
      { layer: "application", matches: (path) => path.startsWith("src/base-") },
    ];

    const report = classifyFrontendSources(
      [{ path: "src/base-overlap.ts", source: "export const overlap = true;" }],
      emptyMembership,
      baseRules,
    );

    expect([...report.classification]).toEqual([]);
    expect(report.errors).toEqual([
      {
        kind: "multiply-classified-production-source",
        sourceFile: "src/base-overlap.ts",
        layers: ["views", "application"],
      },
    ]);
  });

  it("audits frontend packages and stylesheet assets by layer mode and origin", () => {
    const productionSources = [
      "src/app/App.tsx",
      "src/app/main.tsx",
      "src/views/fixture.tsx",
      "src/features/application/fixture.ts",
      "src/features/core/fixture.ts",
      "src/features/domain/fixture.ts",
      "src/services/fixture.ts",
      "src/components/fixture.tsx",
    ].map((path) => ({ path, source: "export {};" }));
    const classification = classifyFrontendSources(productionSources).classification;
    const moduleDependencies = [
      externalModuleDependency("src/views/fixture.tsx", "react", null, "runtime"),
      externalModuleDependency(
        "src/views/fixture.tsx",
        "react",
        null,
        "type-only",
        "module",
        "node_modules/@types/react/index.d.ts::ReactNode",
      ),
      externalModuleDependency(
        "src/views/fixture.tsx",
        "d3",
        null,
        "type-only",
        "module",
        "node_modules/@types/d3/index.d.ts::Selection",
      ),
      externalModuleDependency(
        "src/features/application/fixture.ts",
        "zustand",
        "react::shallow",
        "runtime",
      ),
      externalModuleDependency("src/features/core/fixture.ts", "dockview-react", null, "type-only"),
      externalModuleDependency("src/services/fixture.ts", "@tauri-apps/api", "core", "runtime"),
      externalModuleDependency(
        "src/app/App.tsx",
        "dockview-react",
        "dist::styles::dockview.css",
        "runtime",
        "stylesheet",
      ),
      externalModuleDependency(
        "src/views/fixture.tsx",
        "katex",
        "dist::katex.min.css",
        "runtime",
        "stylesheet",
      ),
      externalModuleDependency(
        "src/components/fixture.tsx",
        "katex",
        "dist::katex.min.css",
        "runtime",
        "stylesheet",
      ),
      repositoryAssetDependency("src/app/App.tsx", "src/app/App.css"),
      repositoryAssetDependency("src/app/main.tsx", "src/app/workbench-dockview.css"),
      externalModuleDependency("src/features/domain/fixture.ts", "react", null, "runtime"),
      externalModuleDependency("src/views/fixture.tsx", "zustand", null, "runtime"),
      externalModuleDependency("src/views/fixture.tsx", "@tauri-apps/api", "window", "runtime"),
      externalModuleDependency(
        "src/features/application/fixture.ts",
        "dockview-react",
        null,
        "runtime",
      ),
      externalModuleDependency("src/views/fixture.tsx", "react", "unlisted", "runtime"),
      externalModuleDependency(
        "src/app/App.tsx",
        "dockview-react",
        "dist::styles::other.css",
        "runtime",
        "stylesheet",
      ),
      externalModuleDependency(
        "src/views/fixture.tsx",
        "dockview-react",
        "dist::styles::dockview.css",
        "runtime",
        "stylesheet",
      ),
      repositoryAssetDependency("src/views/fixture.tsx", "src/app/App.css"),
      externalModuleDependency(
        "src/features/application/fixture.ts",
        "tailwindcss",
        null,
        "runtime",
      ),
      externalModuleDependency(
        "src/features/application/fixture.ts",
        "tailwindcss",
        null,
        "type-only",
      ),
      externalModuleDependency("src/views/fixture.tsx", "@types/react", null, "type-only"),
      externalModuleDependency("src/views/fixture.tsx", "@types/d3", null, "type-only"),
      externalModuleDependency("src/views/fixture.tsx", "vitest", null, "runtime"),
      externalModuleDependency("src/views/fixture.tsx", "mystery-package", null, "runtime"),
      externalModuleDependency("src/views/fixture.tsx", "toString", null, "runtime"),
    ];
    const stylesheetDependencies = [
      stylesheetExternalDependency("src/app/App.css", "tailwindcss", null),
      stylesheetExternalDependency("src/app/App.css", "tw-animate-css", null),
      stylesheetExternalDependency("src/app/App.css", "shadcn", "tailwind.css"),
      stylesheetExternalDependency("src/app/App.css", "@fontsource-variable/inter", null),
      stylesheetExternalDependency("src/app/workbench-dockview.css", "tailwindcss", null),
    ];
    const stylesheetGraph: ResolvedStylesheetGraph = {
      repositoryStylesheets: ["src/app/App.css", "src/app/workbench-dockview.css"],
      dependencies: stylesheetDependencies,
      errors: [
        {
          kind: "stylesheet-target-missing",
          sourceFile: "src/app/App.css",
          canonicalTarget: "src/app/missing.css",
        },
        {
          kind: "stylesheet-path-escapes-repository",
          sourceFile: "src/app/App.css",
          writtenSpecifier: "../../../outside.css",
        },
        {
          kind: "unsupported-stylesheet-target",
          sourceFile: "src/app/App.css",
          writtenSpecifier: "./font.woff2",
        },
        { kind: "stylesheet-cycle", cycle: ["src/app/a.css", "src/app/b.css", "src/app/a.css"] },
      ],
    };
    const packageJson = JSON.parse(readFileSync("package.json", "utf8")) as ReadonlyPackageManifest;

    expect(FRONTEND_EXTERNAL_DEPENDENCY_POLICY.declaredRuntimePackages).toEqual(
      Object.keys(packageJson.dependencies).sort(),
    );
    expect(FRONTEND_EXTERNAL_DEPENDENCY_POLICY.declaredBuildOnlyPackages).toEqual(["tailwindcss"]);
    expect(FRONTEND_ASSET_DEPENDENCY_POLICY.uses).toEqual([
      {
        sourceLayer: "app-composition",
        mode: "runtime",
        dependencyKind: "side-effect-import",
        resourceKind: "stylesheet",
        consumerSourceFile: "src/app/App.tsx",
        repositoryRelativeAssetPath: "src/app/App.css",
      },
      {
        sourceLayer: "app-composition",
        mode: "runtime",
        dependencyKind: "side-effect-import",
        resourceKind: "stylesheet",
        consumerSourceFile: "src/app/main.tsx",
        repositoryRelativeAssetPath: "src/app/workbench-dockview.css",
      },
    ]);

    const assetReport = auditFrontendAssetDependencies(
      {
        productionSources,
        moduleDependencies,
        stylesheetGraph,
      },
      classification,
      FRONTEND_ASSET_DEPENDENCY_POLICY,
    );
    expect(assetReport.stylesheetLayers).toEqual(
      new Map([
        ["src/app/App.css", "app-composition"],
        ["src/app/workbench-dockview.css", "app-composition"],
      ]),
    );
    expect(assetReport.findings).toEqual([
      expect.objectContaining({
        ruleId: "frontend.asset.consumer-path",
        repositoryRelativeSourceFile: "src/views/fixture.tsx",
        canonicalOriginTarget: "repository-asset:src/app/App.css",
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
    expect(externalReport.evaluated).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          packageName: "react",
          mode: "type-only",
          declarationScope: "production",
          allowed: true,
        }),
        expect.objectContaining({
          packageName: "d3",
          mode: "type-only",
          declarationScope: "production",
          allowed: true,
        }),
        expect.objectContaining({
          packageName: "tailwindcss",
          mode: "build-style",
          declarationScope: "development",
          allowed: true,
        }),
        expect.objectContaining({
          packageName: "@tauri-apps/api",
          canonicalSubpath: "core",
          allowed: true,
        }),
      ]),
    );
    expect(externalReport.findings.map(({ ruleId }) => ruleId)).toEqual([
      "frontend.external.runtime-subpath",
      "frontend.external.build-style-consumer",
      "frontend.external.runtime-source-layer",
      "frontend.external.runtime-source-layer",
      "frontend.external.runtime-resource-kind",
      "frontend.external.runtime-source-layer",
      "frontend.external.runtime-subpath",
      "frontend.external.runtime-subpath",
    ]);
    expect(externalReport.errors).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: "development-dependency-in-production",
          packageName: "tailwindcss",
        }),
        expect.objectContaining({
          kind: "development-dependency-in-production",
          packageName: "@types/react",
        }),
        expect.objectContaining({
          kind: "development-dependency-in-production",
          packageName: "@types/d3",
        }),
        expect.objectContaining({
          kind: "development-dependency-in-production",
          packageName: "vitest",
        }),
        expect.objectContaining({
          kind: "unknown-external-package",
          packageName: "mystery-package",
        }),
        expect.objectContaining({ kind: "unknown-external-package", packageName: "toString" }),
      ]),
    );
    expect(
      externalReport.errors.filter(({ kind }) => kind === "development-dependency-in-production"),
    ).toHaveLength(5);

    const duplicateExternalPolicy = {
      ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
      uses: [
        ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses,
        FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses[0],
      ],
    };
    expect(
      auditFrontendExternalDependencies(
        [],
        classification,
        assetReport.stylesheetLayers,
        packageJson,
        duplicateExternalPolicy,
      ).errors,
    ).toContainEqual(expect.objectContaining({ kind: "invalid-external-policy-row" }));
    const duplicateSubpathPolicy = {
      ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
      uses: FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses.map((row, index) =>
        index === 0 ? { ...row, canonicalSubpaths: [null, null] } : row,
      ),
    };
    expect(
      auditFrontendExternalDependencies(
        [],
        classification,
        assetReport.stylesheetLayers,
        packageJson,
        duplicateSubpathPolicy,
      ).errors,
    ).toContainEqual(
      expect.objectContaining({
        kind: "invalid-external-policy-row",
        reason: "duplicate-subpath",
      }),
    );
    const unsupportedModePolicy = {
      ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
      uses: FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses.map((row, index) =>
        index === 0 ? { ...row, mode: "test-only" } : row,
      ),
    } as unknown as ExternalDependencyPolicy;
    expect(
      auditFrontendExternalDependencies(
        [],
        classification,
        assetReport.stylesheetLayers,
        packageJson,
        unsupportedModePolicy,
      ).errors,
    ).toContainEqual(
      expect.objectContaining({
        kind: "invalid-external-policy-row",
        reason: "unsupported-mode",
      }),
    );
    const missingBuildConsumerPolicy = {
      ...FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
      uses: FRONTEND_EXTERNAL_DEPENDENCY_POLICY.uses.map((row) =>
        row.mode === "build-style" && row.packageName === "tailwindcss"
          ? { ...row, consumerSourceFile: "src/app/missing.css" }
          : row,
      ),
    };
    expect(
      auditFrontendExternalDependencies(
        [],
        classification,
        assetReport.stylesheetLayers,
        packageJson,
        missingBuildConsumerPolicy,
      ).errors,
    ).toContainEqual(
      expect.objectContaining({
        kind: "invalid-external-policy-row",
        reason: "invalid-build-style-consumer",
      }),
    );
    const duplicateAssetPolicy = {
      uses: [...FRONTEND_ASSET_DEPENDENCY_POLICY.uses, FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0]],
    };
    expect(
      auditFrontendAssetDependencies(
        {
          productionSources,
          moduleDependencies,
          stylesheetGraph: { ...stylesheetGraph, errors: [] },
        },
        classification,
        duplicateAssetPolicy,
      ).errors,
    ).toContainEqual(expect.objectContaining({ kind: "invalid-asset-policy-row" }));
    const typeOnlyAssetPolicy = {
      uses: [{ ...FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0], mode: "type-only" }],
    } as unknown as AssetDependencyPolicy;
    expect(
      auditFrontendAssetDependencies(
        {
          productionSources,
          moduleDependencies,
          stylesheetGraph: { ...stylesheetGraph, errors: [] },
        },
        classification,
        typeOnlyAssetPolicy,
      ).errors,
    ).toContainEqual(
      expect.objectContaining({
        kind: "invalid-asset-policy-row",
        reason: "unsupported-mode",
      }),
    );
    const stylesheetRuntimeConsumerPolicy: AssetDependencyPolicy = {
      uses: [
        {
          ...FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0],
          consumerSourceFile: "src/app/App.css",
        },
      ],
    };
    expect(
      auditFrontendAssetDependencies(
        {
          productionSources,
          moduleDependencies,
          stylesheetGraph: { ...stylesheetGraph, errors: [] },
        },
        classification,
        stylesheetRuntimeConsumerPolicy,
      ).errors,
    ).toContainEqual(
      expect.objectContaining({
        kind: "invalid-asset-policy-row",
        reason: "runtime-asset-consumer-not-typescript",
      }),
    );

    const conflictModules = [
      repositoryAssetDependency("src/app/App.tsx", "src/app/shared.css"),
      repositoryAssetDependency("src/views/fixture.tsx", "src/app/shared.css"),
    ];
    const conflictPolicy: AssetDependencyPolicy = {
      uses: [
        {
          ...FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0],
          repositoryRelativeAssetPath: "src/app/shared.css",
        },
        {
          ...FRONTEND_ASSET_DEPENDENCY_POLICY.uses[0],
          sourceLayer: "views",
          consumerSourceFile: "src/views/fixture.tsx",
          repositoryRelativeAssetPath: "src/app/shared.css",
        },
      ],
    };
    const conflictReport = auditFrontendAssetDependencies(
      {
        productionSources,
        moduleDependencies: conflictModules,
        stylesheetGraph: {
          repositoryStylesheets: ["src/app/shared.css"],
          dependencies: [],
          errors: [],
        },
      },
      classification,
      conflictPolicy,
    );
    expect(conflictReport.stylesheetLayers.has("src/app/shared.css")).toBe(false);
    expect(conflictReport.errors).toContainEqual({
      kind: "stylesheet-layer-conflict",
      sourceFile: "src/app/shared.css",
      inheritedLayers: ["app-composition", "views"],
    });
  });

  it("invalidates nested stylesheet provenance when a parent gains a second layer", () => {
    const productionSources = [
      { path: "src/app/App.tsx", source: "import './shared.css';" },
      { path: "src/views/fixture.tsx", source: "import '../app/shared.css';" },
    ];
    const classification = classifyFrontendSources(productionSources).classification;
    const moduleDependencies = [
      repositoryAssetDependency("src/app/App.tsx", "src/app/shared.css"),
      repositoryAssetDependency("src/views/fixture.tsx", "src/app/shared.css"),
    ];
    const policy: AssetDependencyPolicy = {
      uses: [
        {
          sourceLayer: "app-composition",
          mode: "runtime",
          dependencyKind: "side-effect-import",
          resourceKind: "stylesheet",
          consumerSourceFile: "src/app/App.tsx",
          repositoryRelativeAssetPath: "src/app/shared.css",
        },
        {
          sourceLayer: "views",
          mode: "runtime",
          dependencyKind: "side-effect-import",
          resourceKind: "stylesheet",
          consumerSourceFile: "src/views/fixture.tsx",
          repositoryRelativeAssetPath: "src/app/shared.css",
        },
        {
          sourceLayer: "app-composition",
          mode: "build-style",
          dependencyKind: "stylesheet-import",
          resourceKind: "stylesheet",
          consumerSourceFile: "src/app/shared.css",
          repositoryRelativeAssetPath: "src/app/nested.css",
        },
      ],
    };

    const report = auditFrontendAssetDependencies(
      {
        productionSources,
        moduleDependencies,
        stylesheetGraph: {
          repositoryStylesheets: ["src/app/nested.css", "src/app/shared.css"],
          dependencies: [stylesheetAssetDependency("src/app/shared.css", "src/app/nested.css")],
          errors: [],
        },
      },
      classification,
      policy,
    );

    expect([...report.stylesheetLayers]).toEqual([]);
    expect(report.findings).toEqual([]);
    expect(report.errors).toEqual([
      {
        kind: "stylesheet-layer-conflict",
        sourceFile: "src/app/shared.css",
        inheritedLayers: ["app-composition", "views"],
      },
    ]);
  });

  it("resolves every module dependency to its canonical origin", () => {
    withIsolatedTypeScriptProject(compilerSources, (context) => {
      const sourcePaths = [...compilerSources.keys()].filter(
        (path) => path.startsWith("src/") && !path.endsWith(".d.ts"),
      );
      const resolved = sourcePaths.flatMap((path) =>
        resolvedModuleDependencies(context, architectureSource(path)),
      );

      expect(resolved).toContainEqual(
        expect.objectContaining({
          repositoryRelativeSourceFile: "src/views/fixture.tsx",
          importedSymbol: "approvedRead",
          canonicalOriginTarget: "src/features/core/fixture/read.ts::approvedRead",
        }),
      );
      expect(resolved).toContainEqual(
        expect.objectContaining({
          kind: "import-type",
          mode: "type-only",
          importedSymbol: "Contract",
          canonicalOriginTarget: "src/features/domain/fixture/contract.ts::Contract",
        }),
      );
      expect(resolved).toContainEqual(
        expect.objectContaining({
          kind: "dynamic-import",
          mode: "runtime",
          importedSymbol: null,
          canonicalOriginTarget: "src/views/runtime.ts",
        }),
      );
      expect(resolved.filter((dependency) => dependency.kind === "import-type")).toHaveLength(1);
      expect(resolved.filter((dependency) => dependency.kind === "dynamic-import")).toHaveLength(1);

      expect(resolved).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            kind: "require",
            mode: "runtime",
            canonicalOriginTarget: "src/views/runtime.ts",
          }),
          expect.objectContaining({
            kind: "import-equals",
            mode: "runtime",
            canonicalOriginTarget: "src/views/runtime.ts",
          }),
          expect.objectContaining({
            kind: "export-assignment",
            mode: "runtime",
            canonicalOriginTarget: "src/views/runtime.ts",
          }),
          expect.objectContaining({
            kind: "static-import",
            mode: "runtime",
            importedSymbol: "runtimeValue",
            canonicalOriginTarget: "src/features/domain/fixture/mixed.ts::runtimeValue",
          }),
          expect.objectContaining({
            kind: "static-import",
            mode: "type-only",
            importedSymbol: "MixedContract",
            canonicalOriginTarget: "src/features/domain/fixture/mixed.ts::MixedContract",
          }),
          expect.objectContaining({
            kind: "re-export",
            mode: "type-only",
            importedSymbol: "Contract",
            canonicalOriginTarget: "src/features/domain/fixture/contract.ts::Contract",
          }),
          expect.objectContaining({
            kind: "static-import",
            importedSymbol: "Contract",
            canonicalOriginTarget: "src/features/domain/fixture/contract.ts::Contract",
          }),
        ]),
      );

      const repositoryDependencies = resolved.filter(
        ({ origin }) => origin.kind === "repository-module",
      );
      expect(
        repositoryDependencies.every(({ origin, symbolDeclarationTarget }) => {
          if (origin.kind !== "repository-module") return false;
          return (
            origin.declarationTarget.startsWith("src/") &&
            (symbolDeclarationTarget === null || symbolDeclarationTarget.startsWith("src/"))
          );
        }),
      ).toBe(true);

      expect(resolved).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            mode: "type-only",
            canonicalOriginTarget: "external:react",
            symbolDeclarationTarget: "node_modules/@types/react/index.d.ts::ReactNode",
            origin: {
              kind: "external",
              dependency: { packageName: "react", canonicalSubpath: null, resourceKind: "module" },
            },
          }),
          expect.objectContaining({
            mode: "type-only",
            canonicalOriginTarget: "external:d3",
            symbolDeclarationTarget: "node_modules/@types/d3/index.d.ts::Selection",
            origin: {
              kind: "external",
              dependency: { packageName: "d3", canonicalSubpath: null, resourceKind: "module" },
            },
          }),
          expect.objectContaining({
            canonicalOriginTarget: "external:@types/react",
            origin: {
              kind: "external",
              dependency: {
                packageName: "@types/react",
                canonicalSubpath: null,
                resourceKind: "module",
              },
            },
          }),
          expect.objectContaining({
            canonicalOriginTarget: "external:@types/d3",
            origin: {
              kind: "external",
              dependency: {
                packageName: "@types/d3",
                canonicalSubpath: null,
                resourceKind: "module",
              },
            },
          }),
          expect.objectContaining({ canonicalOriginTarget: "external:react-dom::client" }),
          expect.objectContaining({ canonicalOriginTarget: "external:@tauri-apps/api::window" }),
          expect.objectContaining({
            repositoryRelativeSourceFile: "src/features/application/i18n-user.ts",
            canonicalOriginTarget: "external:i18next",
            symbolDeclarationTarget: "node_modules/i18next/index.d.ts::i18n",
          }),
          expect.objectContaining({
            kind: "side-effect-import",
            mode: "runtime",
            canonicalOriginTarget: "external:dockview-react::dist::styles::dockview.css",
            origin: {
              kind: "external",
              dependency: {
                packageName: "dockview-react",
                canonicalSubpath: "dist::styles::dockview.css",
                resourceKind: "stylesheet",
              },
            },
          }),
          expect.objectContaining({
            canonicalOriginTarget: "external:katex::dist::katex.min.css",
          }),
          expect.objectContaining({
            canonicalOriginTarget: "repository-asset:src/views/fixture.css",
            origin: {
              kind: "repository-asset",
              asset: {
                repositoryRelativeAssetPath: "src/views/fixture.css",
                resourceKind: "stylesheet",
              },
            },
          }),
        ]),
      );

      const stylesheetGraph = resolvedStylesheetDependencies(
        resolve("."),
        resolved,
        new FixtureTextReader(stylesheetSources),
      );
      expect(stylesheetGraph.repositoryStylesheets).toEqual([
        "src/views/cycle-a.css",
        "src/views/cycle-b.css",
        "src/views/fixture.css",
        "src/views/malformed.css",
        "src/views/nested.css",
        "src/views/url-target.css",
      ]);
      expect(stylesheetGraph.dependencies).toEqual(
        expect.arrayContaining([
          ...[
            ["external:tailwindcss", "tailwindcss"],
            ["external:tw-animate-css", "tw-animate-css"],
            ["external:shadcn::tailwind.css", "shadcn"],
            ["external:@fontsource-variable/inter", "@fontsource-variable/inter"],
          ].map(([canonicalOriginTarget, packageName]) =>
            expect.objectContaining({
              repositoryRelativeSourceFile: "src/views/fixture.css",
              fullyQualifiedOwner: "stylesheet:src/views/fixture.css",
              kind: "stylesheet-import",
              mode: "build-style",
              canonicalOriginTarget,
              origin: expect.objectContaining({
                kind: "external",
                dependency: expect.objectContaining({ packageName, resourceKind: "stylesheet" }),
              }),
            }),
          ),
          expect.objectContaining({
            repositoryRelativeSourceFile: "src/views/fixture.css",
            kind: "stylesheet-url",
            mode: "build-style",
            canonicalOriginTarget: "repository-asset:src/views/url-target.css",
          }),
        ]),
      );
      expect(stylesheetGraph.dependencies.every(({ line, column }) => line > 0 && column > 0)).toBe(
        true,
      );
      expect(stylesheetGraph.errors.map(({ kind }) => kind)).toEqual(
        expect.arrayContaining([
          "stylesheet-parse-failure",
          "stylesheet-cycle",
          "stylesheet-path-escapes-repository",
          "stylesheet-target-missing",
          "unsupported-stylesheet-target",
        ]),
      );
    });

    const invalidModuleSources = [
      [
        "src/views/nonliteral.ts",
        "const target = getTarget(); void import(target);",
        "nonliteral-module-specifier",
      ],
      ["src/views/parent-package.ts", "import 'react/../secret';", "invalid-external-specifier"],
      ["src/views/backslash-package.ts", "import 'react\\\\secret';", "invalid-external-specifier"],
      ["src/views/encoded-package.ts", "import 'react/%2fsecret';", "invalid-external-specifier"],
      [
        "src/views/missing-package.ts",
        "import 'not-a-real-package';",
        "unresolved-module-dependency",
      ],
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

  it("collects nested dependencies inside recognized import syntax", () => {
    const sources = new Map<string, string>([
      [
        "src/views/nested.ts",
        `
        export type Nested = import('./outer').Box<import('./inner').Thing>;
        void import('./outer', { with: { type: import('./inner') } });
        export default import('./outer', (import('./inner'), {}));
      `,
      ],
      ["src/views/outer.ts", "export interface Box<T> { readonly value: T; }"],
      ["src/views/inner.ts", "export interface Thing { readonly value: string; }"],
    ]);

    withIsolatedTypeScriptProject(sources, (context) => {
      const resolved = resolvedModuleDependencies(context, {
        path: "src/views/nested.ts",
        source: sources.get("src/views/nested.ts")!,
      });

      expect(
        resolved.map(({ kind, canonicalOriginTarget }) => ({
          kind,
          canonicalOriginTarget,
        })),
      ).toEqual([
        {
          kind: "import-type",
          canonicalOriginTarget: "src/views/outer.ts::Box",
        },
        {
          kind: "import-type",
          canonicalOriginTarget: "src/views/inner.ts::Thing",
        },
        {
          kind: "dynamic-import",
          canonicalOriginTarget: "src/views/outer.ts",
        },
        {
          kind: "dynamic-import",
          canonicalOriginTarget: "src/views/inner.ts",
        },
        {
          kind: "export-assignment",
          canonicalOriginTarget: "src/views/outer.ts",
        },
        {
          kind: "dynamic-import",
          canonicalOriginTarget: "src/views/inner.ts",
        },
      ]);
    });
  });

  it("rejects forged declarations outside the exact audit source root", () => {
    const isolatedPath = "src/views/out-of-root.ts";
    const isolatedSource = "import { forged } from '../../sibling/src/forged'; void forged;";
    withIsolatedTypeScriptProject(
      new Map([
        [isolatedPath, isolatedSource],
        ["sibling/src/forged.ts", "export const forged = true;"],
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
          kind: "unresolved-module-dependency",
          sourceFile: isolatedPath,
          writtenSpecifier: "../../sibling/src/forged",
        });
      },
    );

    const sandbox = mkdtempSync(join(tmpdir(), "yssbi-typescript-source-root-"));
    const importerPath = join(sandbox, "src", "views", "screen.ts");
    const forgedPath = join(sandbox, "run-1", "src", "forged.ts");
    const configPath = join(sandbox, "tsconfig.json");
    const source = "import { forged } from '../../run-1/src/forged'; void forged;";
    mkdirSync(join(sandbox, "src", "views"), { recursive: true });
    mkdirSync(join(sandbox, "run-1", "src"), { recursive: true });
    writeFileSync(importerPath, source);
    writeFileSync(forgedPath, "export const forged = true;");
    writeFileSync(
      configPath,
      JSON.stringify({
        compilerOptions: { noLib: true, strict: true, target: "esnext" },
        files: ["src/views/screen.ts", "run-1/src/forged.ts"],
      }),
    );

    let failure: unknown;
    try {
      withProductionTypeScriptProject((context) => {
        expect(productionTypeScriptSources(context).map(({ path }) => path)).toEqual([
          "src/views/screen.ts",
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
      kind: "unresolved-module-dependency",
      sourceFile: importerPath,
      writtenSpecifier: "../../run-1/src/forged",
    });
  });

  it("fails closed for recognized dependencies without literal specifiers", () => {
    const cases = [
      ["src/views/nonliteral-import-type.ts", "type Contract = import(Target).Contract;"],
      ["src/views/missing-dynamic-import-argument.ts", "const loaded = import(); void loaded;"],
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
          kind: "nonliteral-module-specifier",
          sourceFile: path,
          writtenSpecifier: null,
        });
      }
    });
  });

  it("inventories the complete frontend production tree", () => {
    const isolatedSources = new Map<string, string>([
      ["src/app/bootstrap.ts", "export const bootstrap = true;"],
      ["src/views/screen.tsx", "export const screen = null;"],
      ["src/features/application/useCase.ts", "export const useCase = true;"],
      ["src/services/backend.ts", "export const backend = true;"],
      ["src/components/control.tsx", "export const control = null;"],
      ["src/shared/utils/kept.ts", "export const kept = true;"],
      ["src/utils/diagnostic.ts", "export const diagnostic = true;"],
      ["src/tests/helper.ts", "export const testHelper = true;"],
      ["src/shared/behavior.test.ts", "export const test = true;"],
      ["src/shared/namedFixture.ts", "export const fixture = true;"],
      ["src/shared/generated.generated.ts", "export const generated = true;"],
      ["src/shared/contracts.d.ts", "export interface Contract {}"],
    ]);

    withIsolatedTypeScriptProject(isolatedSources, (context) => {
      expect(productionTypeScriptSources(context).map(({ path }) => path)).toEqual([
        "src/app/bootstrap.ts",
        "src/components/control.tsx",
        "src/features/application/useCase.ts",
        "src/services/backend.ts",
        "src/shared/utils/kept.ts",
        "src/utils/diagnostic.ts",
        "src/views/screen.tsx",
      ]);
      expect(
        productionTypeScriptSources(context).every(
          ({ path }) => path.startsWith("src/") && !path.includes("\\"),
        ),
      ).toBe(true);
    });

    withProductionTypeScriptProject((context) => {
      const productionPaths = productionTypeScriptSources(context).map(({ path }) => path);
      for (const root of [
        "src/app/",
        "src/modules/",
        "src/features/",
        "src/services/",
        "src/components/",
        "src/shared/",
        "src/utils/",
      ]) {
        expect(
          productionPaths.some((path) => path.startsWith(root)),
          root,
        ).toBe(true);
      }
      expect(productionPaths.some((path) => path.startsWith("src/views/"))).toBe(false);
    });
  });

  it("fails closed for an unresolvable external stylesheet package", () => {
    const path = "src/views/missingStylesheetPackage.ts";
    const source = "import 'not-a-real-package/theme.css';";
    let failure: unknown;

    withIsolatedTypeScriptProject({ [path]: source }, (context) => {
      try {
        resolvedModuleDependencies(context, { path, source });
      } catch (error) {
        failure = error;
      }
    });

    expect(failure).toBeInstanceOf(ModuleDependencyResolutionError);
    expect(failure).toMatchObject({
      kind: "unresolved-module-dependency",
      sourceFile: path,
      writtenSpecifier: "not-a-real-package/theme.css",
    });
  });

  it("emits declaration facts for empty named imports and exports", () => {
    const path = "src/views/emptyNamedDeclarations.ts";
    const sources = new Map<string, string>([
      [
        path,
        `
        import {} from './runtime';
        import type {} from './contract';
        export {} from './runtime';
        export type {} from './contract';
      `,
      ],
      ["src/views/runtime.ts", "export const value = 1;"],
      ["src/views/contract.ts", "export interface Contract {}"],
    ]);

    withIsolatedTypeScriptProject(sources, (context) => {
      expect(resolvedModuleDependencies(context, { path, source: sources.get(path)! })).toEqual([
        expect.objectContaining({
          kind: "static-import",
          mode: "runtime",
          importedSymbol: null,
          canonicalOriginTarget: "src/views/runtime.ts",
          symbolDeclarationTarget: "src/views/runtime.ts",
        }),
        expect.objectContaining({
          kind: "static-import",
          mode: "type-only",
          importedSymbol: null,
          canonicalOriginTarget: "src/views/contract.ts",
          symbolDeclarationTarget: "src/views/contract.ts",
        }),
        expect.objectContaining({
          kind: "re-export",
          mode: "runtime",
          importedSymbol: null,
          canonicalOriginTarget: "src/views/runtime.ts",
          symbolDeclarationTarget: "src/views/runtime.ts",
        }),
        expect.objectContaining({
          kind: "re-export",
          mode: "type-only",
          importedSymbol: null,
          canonicalOriginTarget: "src/views/contract.ts",
          symbolDeclarationTarget: "src/views/contract.ts",
        }),
      ]);
    });
  });

  it("reports each invalid stylesheet input exactly without granting a dependency", () => {
    const path = "src/views/fixture.css";
    const cases = [
      {
        name: "unterminated quoted import",
        source: '@import "unterminated.css;',
        error: { kind: "stylesheet-parse-failure", sourceFile: path, line: 1, column: 9 },
      },
      {
        name: "nonliteral import",
        source: "@import url(var(--theme));",
        error: { kind: "stylesheet-parse-failure", sourceFile: path, line: 1, column: 16 },
      },
      {
        name: "remote import",
        source: '@import "https://example.invalid/theme.css";',
        error: {
          kind: "unsupported-stylesheet-target",
          sourceFile: path,
          writtenSpecifier: "https://example.invalid/theme.css",
        },
      },
      {
        name: "repository escape",
        source: '@import "../../../outside.css";',
        error: {
          kind: "stylesheet-path-escapes-repository",
          sourceFile: path,
          writtenSpecifier: "../../../outside.css",
        },
      },
      {
        name: "unsupported parent stylesheet",
        source: '@import "../parent.css";',
        error: {
          kind: "unsupported-stylesheet-target",
          sourceFile: path,
          writtenSpecifier: "../parent.css",
        },
      },
      {
        name: "normalized parent stylesheet",
        source: '@import "./../parent.css";',
        error: {
          kind: "unsupported-stylesheet-target",
          sourceFile: path,
          writtenSpecifier: "./../parent.css",
        },
      },
      {
        name: "encoded package separator",
        source: '@import "react/%2fsecret";',
        error: {
          kind: "unsupported-stylesheet-target",
          sourceFile: path,
          writtenSpecifier: "react/%2fsecret",
        },
      },
      {
        name: "missing repository stylesheet",
        source: '@import "./missing.css";',
        error: {
          kind: "stylesheet-target-missing",
          sourceFile: path,
          canonicalTarget: "src/views/missing.css",
        },
      },
      {
        name: "unsupported repository asset",
        source: '@import "./font.woff2";',
        error: {
          kind: "unsupported-stylesheet-target",
          sourceFile: path,
          writtenSpecifier: "./font.woff2",
        },
      },
      {
        name: "quoted backslash",
        source: '@import "react\\secret";',
        error: {
          kind: "unsupported-stylesheet-target",
          sourceFile: path,
          writtenSpecifier: "react\\secret",
        },
      },
    ] as const;

    for (const fixture of cases) {
      const graph = resolvedStylesheetDependencies(
        resolve("."),
        [stylesheetRoot(path)],
        new FixtureTextReader(
          new Map([
            [path, fixture.source],
            ["src/parent.css", ".parent {}"],
          ]),
        ),
      );
      expect(graph.dependencies, fixture.name).toEqual([]);
      expect(graph.errors, fixture.name).toEqual([fixture.error]);
    }
  });

  it("reads repository text only through real root-bounded src paths", () => {
    const sandbox = mkdtempSync(join(tmpdir(), "yssbi-repository-reader-"));
    const repositoryRoot = join(sandbox, "repository");
    const outsideRoot = join(sandbox, "outside");
    const sourceRoot = join(repositoryRoot, "src");
    mkdirSync(join(sourceRoot, "app"), { recursive: true });
    mkdirSync(outsideRoot);
    writeFileSync(join(sourceRoot, "app", "App.css"), ".app {}");
    writeFileSync(join(outsideRoot, "secret.css"), ".secret {}");
    symlinkSync(outsideRoot, join(sourceRoot, "escape"), "junction");

    try {
      const reader = createRepositoryTextReader(repositoryRoot);
      expect(reader.readRepositoryText("src/app/App.css")).toBe(".app {}");
      expect(reader.readRepositoryText(resolve(sourceRoot, "app", "App.css"))).toBeNull();
      expect(reader.readRepositoryText("package.json")).toBeNull();
      expect(reader.readRepositoryText("../outside/secret.css")).toBeNull();
      expect(reader.readRepositoryText("src/escape/secret.css")).toBeNull();
    } finally {
      rmSync(sandbox, { recursive: true, force: true });
    }
  });

  it("builds the real App and workbench stylesheet graph through the repository reader", () => {
    withProductionTypeScriptProject((context) => {
      const sources = new Map(
        productionTypeScriptSources(context).map((source) => [source.path, source]),
      );
      const moduleDependencies = ["src/app/App.tsx", "src/app/main.tsx"].flatMap((path) =>
        resolvedModuleDependencies(context, sources.get(path)!),
      );
      const graph = resolvedStylesheetDependencies(
        resolve("."),
        moduleDependencies,
        createRepositoryTextReader(resolve(".")),
      );

      expect(graph.repositoryStylesheets).toEqual([
        "src/app/App.css",
        "src/app/workbench-dockview.css",
      ]);
      expect(graph.dependencies.map(({ canonicalOriginTarget }) => canonicalOriginTarget)).toEqual([
        "external:tailwindcss",
        "external:tw-animate-css",
        "external:shadcn::tailwind.css",
        "external:@fontsource-variable/inter",
      ]);
      expect(graph.errors).toEqual([]);
    });
  });
});
