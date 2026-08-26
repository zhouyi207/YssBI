import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

import { productionTypeScriptSources } from '@/tests/helpers/productionSourceAudit';
import {
  withIsolatedTypeScriptProject,
  withProductionTypeScriptProject,
} from '@/tests/helpers/typescriptAudit';
import {
  FRONTEND_ASSET_DEPENDENCY_POLICY,
} from './frontendAssetDependencyPolicy';
import { auditFrontendArchitectureDependencies } from './frontendArchitectureAudit';
import {
  FRONTEND_ARCHITECTURE_DEBT,
  compareExactFrontendDebt,
} from './frontendArchitectureDebt';
import type {
  FrontendArchitecturePolicy,
  ReadonlyPackageManifest,
} from './frontendArchitectureModel';
import { FRONTEND_ARCHITECTURE_POLICY } from './frontendArchitecturePolicy';
import {
  FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
} from './frontendExternalDependencyPolicy';
import { createRepositoryTextReader } from './frontendArchitectureModel';
import { auditFrontendSemantics } from './frontendSemanticAudit';

const fixtureSources = new Map<string, string>([
  ['src/services/ipc/invokeCommand.ts', `
    import { invoke } from '@tauri-apps/api/core';
    export const invokeCommand = (command: string) => invoke(command);
  `],
  ['src/views/RawInvokeView.tsx', `
    import { invoke } from '@tauri-apps/api/core';
    export const RawInvokeView = () => invoke('raw_view_command');
  `],
  ['src/services/tauriCoreBarrel.ts', `
    import { invoke } from '@tauri-apps/api/core';
    export { invoke };
  `],
  ['src/services/rawInvokeBarrelConsumer.ts', `
    import { invoke } from './tauriCoreBarrel';
    export const callRawBackend = () => invoke('raw_barrel_command');
  `],
  ['src/services/platform/pathDialog.ts', `
    import { open } from '@tauri-apps/plugin-dialog';
    export const selectPath = () => open();
  `],
  ['src/views/RawDialogView.tsx', `
    import { open } from '@tauri-apps/plugin-dialog';
    export const RawDialogView = () => open();
  `],
  ['src/features/core/fixture/RawDialogCore.ts', `
    import { open } from '@tauri-apps/plugin-dialog';
    export const openFromCore = () => open();
  `],
  ['src/features/core/fixture/projectionPublisher.ts', `
    export const projectionPublisher = { publish: (_value: unknown) => undefined };
  `],
  ['src/features/core/fixture/projectionPublisherBarrel.ts', `
    export { projectionPublisher } from './projectionPublisher';
  `],
  ['src/views/PublisherView.tsx', `
    import { projectionPublisher } from '../features/core/fixture/projectionPublisherBarrel';
    projectionPublisher.publish('value');
  `],
  ['src/features/core/fixture/projectionRead.ts', `
    export const projectionRead = {
      getSnapshot: () => ({ revision: 1 }),
      setState: (_value: unknown) => undefined,
    };
  `],
  ['src/views/ProjectionReadView.tsx', `
    import { projectionRead } from '../features/core/fixture/projectionRead';
    projectionRead.getSnapshot();
    projectionRead.setState({ revision: 2 });
  `],
  ['src/features/core/fixture/workbenchDockviewPort.ts', `
    export interface WorkbenchDockviewPort {
      getSnapshot(): Readonly<{ revision: number }>;
      openEditor(resourceRef: string): Promise<void>;
    }
  `],
  ['src/views/TypedProjectionReadView.tsx', `
    import type { WorkbenchDockviewPort } from '../features/core/fixture/workbenchDockviewPort';
    export function inspectPort(port: WorkbenchDockviewPort): void {
      port.getSnapshot();
      void port.openEditor('events/main');
    }
  `],
  ['src/features/core/fixture/projectionStore.ts', `
    export const useProjectionStore = { setState: (_value: unknown) => undefined };
  `],
  ['src/features/core/fixture/projectionStoreBarrel.ts', `
    export { useProjectionStore } from './projectionStore';
  `],
  ['src/services/fixtureProjectionService.ts', `
    import { useProjectionStore } from '../features/core/fixture/projectionStoreBarrel';
    export const publishProjection = () => useProjectionStore.setState({ ready: true });
  `],
  ['src/shared/types/dto/rawWire.ts', `
    export const parseWire = (value: unknown) => value;
  `],
  ['src/features/application/rawWireConsumer.ts', `
    import { invokeCommand } from '../../services/ipc/invokeCommand';
    import { parseWire } from '../../shared/types/dto/rawWire';
    export const loadRaw = async () => parseWire(await invokeCommand('load_raw'));
  `],
  ['src/views/EditorView/Layout/Workspace.tsx', `
    import { DockviewReact } from 'dockview-react';
    export const Workspace = () => <DockviewReact />;
  `],
  ['src/views/LogView/LogWorkspaceDockview.tsx', `
    import { DockviewReact } from 'dockview-react';
    export const LogWorkspaceDockview = () => <DockviewReact />;
  `],
  ['src/views/EditorView/Layout/OtherWorkspace.tsx', `
    import { DockviewReact } from 'dockview-react';
    export const OtherWorkspace = () => <DockviewReact />;
  `],
  ['src/views/EditorView/Layout/NamespaceWorkspace.tsx', `
    import * as Dockview from 'dockview-react';
    export const NamespaceWorkspace = () => <Dockview.DockviewReact />;
  `],
  ['src/views/LogView/OtherLogWorkspace.tsx', `
    import { DockviewReact } from 'dockview-react';
    export const OtherLogWorkspace = () => <DockviewReact />;
  `],
  ['node_modules/@tauri-apps/api/core.d.ts', `
    export declare function invoke<T>(command: string): Promise<T>;
  `],
  ['node_modules/@tauri-apps/plugin-dialog/index.d.ts', `
    export declare function open(): Promise<string | null>;
  `],
  ['node_modules/dockview-react/index.d.ts', `
    export declare function DockviewReact(): unknown;
  `],
]);

const fixturePolicy: FrontendArchitecturePolicy = {
  layerEdges: [],
  capabilities: [
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/fixture/projectionRead.ts',
      exportedSymbols: ['projectionRead'],
      exactConsumers: null,
      memberCapabilities: {
        ProjectionRead: ['getSnapshot'],
      },
    },
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/fixture/workbenchDockviewPort.ts',
      exportedSymbols: ['WorkbenchDockviewPort'],
      exactConsumers: null,
      memberCapabilities: {
        WorkbenchDockviewRead: ['getSnapshot'],
      },
    },
  ],
};

describe('frontend semantic architecture', () => {
  it('reports stable frontend semantic boundary rules', () => {
    withIsolatedTypeScriptProject(fixtureSources, (context) => {
      const findings = auditFrontendSemantics(
        context,
        productionTypeScriptSources(context),
        fixturePolicy,
      );

      expect(findings.map((finding) => ({
        ruleId: finding.ruleId,
        sourceFile: finding.repositoryRelativeSourceFile,
        dependencyKind: finding.dependencyKind,
        canonicalOriginTarget: finding.canonicalOriginTarget,
      }))).toEqual([
        {
          ruleId: 'frontend.application.raw-wire',
          sourceFile: 'src/features/application/rawWireConsumer.ts',
          dependencyKind: 'static-import',
          canonicalOriginTarget: 'src/services/ipc/invokeCommand.ts::invokeCommand',
        },
        {
          ruleId: 'frontend.application.raw-wire',
          sourceFile: 'src/features/application/rawWireConsumer.ts',
          dependencyKind: 'static-import',
          canonicalOriginTarget: 'src/shared/types/dto/rawWire.ts::parseWire',
        },
        {
          ruleId: 'frontend.dialog.raw',
          sourceFile: 'src/features/core/fixture/RawDialogCore.ts',
          dependencyKind: 'static-import',
          canonicalOriginTarget: 'external:@tauri-apps/plugin-dialog',
        },
        {
          ruleId: 'frontend.dialog.raw',
          sourceFile: 'src/views/RawDialogView.tsx',
          dependencyKind: 'static-import',
          canonicalOriginTarget: 'external:@tauri-apps/plugin-dialog',
        },
        {
          ruleId: 'frontend.dockview.nested-constructor',
          sourceFile: 'src/views/LogView/OtherLogWorkspace.tsx',
          dependencyKind: 'constructor',
          canonicalOriginTarget: 'external:dockview-react',
        },
        {
          ruleId: 'frontend.dockview.root-constructor',
          sourceFile: 'src/views/EditorView/Layout/NamespaceWorkspace.tsx',
          dependencyKind: 'constructor',
          canonicalOriginTarget: 'external:dockview-react',
        },
        {
          ruleId: 'frontend.dockview.root-constructor',
          sourceFile: 'src/views/EditorView/Layout/OtherWorkspace.tsx',
          dependencyKind: 'constructor',
          canonicalOriginTarget: 'external:dockview-react',
        },
        {
          ruleId: 'frontend.invoke.raw',
          sourceFile: 'src/services/rawInvokeBarrelConsumer.ts',
          dependencyKind: 'call',
          canonicalOriginTarget: 'external:@tauri-apps/api::core',
        },
        {
          ruleId: 'frontend.invoke.raw',
          sourceFile: 'src/views/RawInvokeView.tsx',
          dependencyKind: 'call',
          canonicalOriginTarget: 'external:@tauri-apps/api::core',
        },
        {
          ruleId: 'frontend.projection-read-mutation',
          sourceFile: 'src/views/ProjectionReadView.tsx',
          dependencyKind: 'property-access',
          canonicalOriginTarget: 'src/features/core/fixture/projectionRead.ts::projectionRead',
        },
        {
          ruleId: 'frontend.projection-read-mutation',
          sourceFile: 'src/views/TypedProjectionReadView.tsx',
          dependencyKind: 'property-access',
          canonicalOriginTarget: 'src/features/core/fixture/workbenchDockviewPort.ts::WorkbenchDockviewPort',
        },
        {
          ruleId: 'frontend.service-projection-write',
          sourceFile: 'src/services/fixtureProjectionService.ts',
          dependencyKind: 'call',
          canonicalOriginTarget: 'src/features/core/fixture/projectionStore.ts::useProjectionStore',
        },
        {
          ruleId: 'frontend.view-core.capability',
          sourceFile: 'src/views/PublisherView.tsx',
          dependencyKind: 'static-import',
          canonicalOriginTarget: 'src/features/core/fixture/projectionPublisher.ts::projectionPublisher',
        },
        {
          ruleId: 'frontend.view-publication',
          sourceFile: 'src/views/PublisherView.tsx',
          dependencyKind: 'property-access',
          canonicalOriginTarget: 'src/features/core/fixture/projectionPublisher.ts::projectionPublisher',
        },
      ]);
    });

    withIsolatedTypeScriptProject(
      new Map([['src/unclassified.ts', 'export const unclassified = true;']]),
      (context) => {
        expect(() => auditFrontendSemantics(
          context,
          productionTypeScriptSources(context),
          fixturePolicy,
        )).toThrowError(
          'Frontend semantic audit requires total source classification: '
          + 'unclassified-production-source:src/unclassified.ts',
        );
      },
    );
  });

  it('frontend production architecture matches dependency and semantic policy', () => {
    const packageJson = JSON.parse(
      readFileSync('package.json', 'utf8'),
    ) as ReadonlyPackageManifest;
    withProductionTypeScriptProject((context) => {
      const dependencyReport = auditFrontendArchitectureDependencies(
        context,
        resolve('.'),
        createRepositoryTextReader(resolve('.')),
        FRONTEND_ARCHITECTURE_POLICY,
        FRONTEND_EXTERNAL_DEPENDENCY_POLICY,
        FRONTEND_ASSET_DEPENDENCY_POLICY,
        packageJson,
      );
      const semanticFindings = auditFrontendSemantics(
        context,
        productionTypeScriptSources(context),
        FRONTEND_ARCHITECTURE_POLICY,
      );
      const debt = compareExactFrontendDebt(
        [...dependencyReport.findings, ...semanticFindings],
        FRONTEND_ARCHITECTURE_DEBT,
      );
      expect({
        unresolvedErrors: dependencyReport.unresolvedErrors,
        classifierErrors: dependencyReport.classification.errors,
        externalErrors: dependencyReport.external.errors,
        assetErrors: dependencyReport.asset.errors,
        debtErrors: debt.errors,
      }).toEqual({
        unresolvedErrors: [],
        classifierErrors: [],
        externalErrors: [],
        assetErrors: [],
        debtErrors: [],
      });
      expect(debt.newOrIncreased).toEqual([]);
      expect(debt.staleOrDecreased).toEqual([]);
    });
  }, 120_000);
});
