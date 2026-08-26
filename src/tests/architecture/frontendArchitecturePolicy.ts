import type { ArchitectureSource } from '@/tests/helpers/moduleDependencyAudit';
import type {
  FrontendArchitecturePolicy,
  FrontendClassificationError,
  FrontendClassificationReport,
  FrontendLayer,
  FrontendLiteralPolicyMembership,
} from './frontendArchitectureModel';

export type {
  FrontendClassificationReport,
  FrontendLayer,
  FrontendLiteralPolicyMembership,
} from './frontendArchitectureModel';

export const FRONTEND_LAYERS = [
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
] as const satisfies readonly FrontendLayer[];

export const FRONTEND_LITERAL_POLICY_MEMBERSHIP: FrontendLiteralPolicyMembership = {
  'app-composition': [],
  views: [],
  application: [],
  core: [
    'src/shared/utils/globalEvent.ts',
  ],
  domain: [],
  services: [
    'src/shared/platform/tauriWebview.ts',
    'src/shared/utils/openExternalUrl.ts',
  ],
  'components-ui': [
    'src/shared/theme/chartTheme.ts',
    'src/shared/theme/dockviewTheme.ts',
  ],
  'wire-schema': [],
  diagnostics: [],
  'pure-shared': [],
};

const LAYER_EDGES = [
  ['app-composition', 'views'],
  ['app-composition', 'application'],
  ['app-composition', 'components-ui'],
  ['app-composition', 'pure-shared'],
  ['views', 'application'],
  ['views', 'components-ui'],
  ['views', 'pure-shared'],
  ['application', 'domain'],
  ['application', 'core'],
  ['application', 'services'],
  ['application', 'components-ui'],
  ['application', 'pure-shared'],
  ['core', 'domain'],
  ['core', 'pure-shared'],
  ['domain', 'pure-shared'],
  ['services', 'wire-schema'],
  ['services', 'pure-shared'],
  ['components-ui', 'domain'],
  ['components-ui', 'pure-shared'],
  ['wire-schema', 'domain'],
  ['wire-schema', 'pure-shared'],
  ['diagnostics', 'wire-schema'],
  ['diagnostics', 'pure-shared'],
] as const satisfies readonly (readonly [FrontendLayer, FrontendLayer])[];

const WORKBENCH_DOCKVIEW_READ_MEMBERS = [
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
] as const;

export const FRONTEND_ARCHITECTURE_POLICY: FrontendArchitecturePolicy = {
  layerEdges: LAYER_EDGES,
  capabilities: [
    {
      sourceLayer: 'app-composition',
      canonicalModule: 'src/features/core/dockview/workbenchDockviewPort.ts',
      exportedSymbols: ['WorkbenchDockviewPort'],
      exactConsumers: null,
      memberCapabilities: {
        WorkbenchDockviewRead: WORKBENCH_DOCKVIEW_READ_MEMBERS,
      },
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/database.ts',
      exportedSymbols: [
        'ColumnInfo',
        'DatabaseDocumentDto',
        'DatabaseImportSourceDTO',
        'DatabaseRecord',
        'DatabaseRow',
        'LoadDatabaseResult',
      ],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/nodeCreationDescriptor.ts',
      exportedSymbols: ['ResourceBoundCreateArgsDto'],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/executionDemand.ts',
      exportedSymbols: ['GraphOutputRefDto'],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/runEvent.ts',
      exportedSymbols: ['RunEvent', 'RunOutputChannelEvent'],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/editorProjection.ts',
      exportedSymbols: [
        'EditorGraphProjectionDto',
        'FunctionEditorProjectionDto',
        'GraphProjectionReplacementDto',
        'NodePositionDto',
        'PortAddressDto',
      ],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/editorMutation.ts',
      exportedSymbols: [
        'EditorGraphMutationDto',
        'FunctionDocumentPatchDto',
        'FunctionSignatureDto',
        'GraphDeltaDto',
        'GraphMutationResultDto',
        'HistoryMutationDto',
        'HistoryStatusDto',
        'MutationRequestDto',
        'ResourceDeltaDto',
        'ResourceKeyDto',
        'ResourceMoveDto',
        'ResourceMutationResultDto',
        'VariableDocumentPatchDto',
      ],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/clipboardSubgraph.ts',
      exportedSymbols: ['ClipboardSubgraphDto'],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/result.ts',
      exportedSymbols: [
        'ResultDescriptor',
        'ResultFailure',
        'ResultPage',
        'ResultPlotKind',
        'ResultPresentation',
        'ResultProgress',
        'ResultReportKind',
      ],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/plotPayload.ts',
      exportedSymbols: [
        'AxisFormat',
        'CorrelationPlotDTO',
        'CorrelogramPlotDTO',
        'HistogramPlotDTO',
        'ParsedPlotPayload',
        'PlotPointDTO',
        'XySeriesPlotDTO',
      ],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/project.ts',
      exportedSymbols: ['LifecycleMutationKind', 'LifecycleMutationResultDto', 'ProjectRecordRow'],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'application',
      canonicalModule: 'src/shared/types/dto/projectComputationSettings.ts',
      exportedSymbols: [
        'ComputationSettingsMutationReceiptDto',
        'ComputationSettingsSnapshotDto',
        'ProjectComputationSettingsDto',
        'StatisticalMissingValuePolicy',
      ],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/dnd/dndContracts.ts',
      exportedSymbols: [
        'DRAG_TYPES',
        'DragType',
        'DROP_TYPES',
        'NodeSpawnTemplate',
        'GraphResourceDragData',
        'NodeTemplateDragData',
        'NodeTemplateDragPayload',
        'GraphResourceDragPayload',
        'CanvasDragPayload',
        'SidebarDragPayload',
        'CanvasDropData',
        'CANVAS_DROP_ZONE_ID_PREFIX',
        'getCanvasDropZoneId',
        'isCanvasDrop',
        'isNodeTemplateDragData',
        'isGraphResourceDragPayload',
        'parseCanvasDragPayload',
        'isSidebarSpawnDrag',
        'isGraphResourceDragState',
        'getSidebarResourceFromDrag',
        'NodeTemplateDragState',
        'GraphResourceDragState',
        'SidebarDragState',
        'isNodeTemplateDragState',
        'getSidebarResourceFromDragState',
      ],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/dnd/dragEventInput.ts',
      exportedSymbols: ['DragModifiers', 'resolveDragClientPoint', 'readDragModifiers'],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/dnd/snapTopLeftToCursorModifier.ts',
      exportedSymbols: ['snapTopLeftToCursor'],
      exactConsumers: null,
      memberCapabilities: null,
    },
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/dockview/workbenchDockviewPort.ts',
      exportedSymbols: ['WorkbenchDockviewPort'],
      exactConsumers: null,
      memberCapabilities: {
        WorkbenchDockviewRead: WORKBENCH_DOCKVIEW_READ_MEMBERS,
      },
    },
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/dockview/workbenchRootBinding.ts',
      exportedSymbols: ['workbenchRootBinding'],
      exactConsumers: ['src/views/EditorView/Layout/Workspace.tsx'],
      memberCapabilities: null,
    },
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/dockview/logsRootBinding.ts',
      exportedSymbols: ['logsRootBinding'],
      exactConsumers: ['src/views/LogView/LogWorkspaceDockview.tsx'],
      memberCapabilities: null,
    },
  ],
};

function normalizeSourcePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/^\.\//, '');
}

function baseLayer(path: string): FrontendLayer | null {
  if (path.startsWith('src/app/')) return 'app-composition';
  if (path.startsWith('src/views/')) return 'views';
  if (path.startsWith('src/features/application/')) return 'application';
  if (path.startsWith('src/features/core/')) return 'core';
  if (path.startsWith('src/features/domain/')) return 'domain';
  if (path.startsWith('src/services/')) return 'services';
  if (path.startsWith('src/components/')) return 'components-ui';
  if (/^src\/shared\/(?:ui|charts|hooks|plot)\//.test(path)) return 'components-ui';
  if (path.startsWith('src/shared/types/dto/')) return 'wire-schema';
  if (path.startsWith('src/utils/')) return 'diagnostics';
  if (path.startsWith('src/lib/') || path.startsWith('src/shared/')) return 'pure-shared';
  return null;
}

function literalSets(
  membership: FrontendLiteralPolicyMembership,
): ReadonlyMap<FrontendLayer, ReadonlySet<string>> {
  return new Map(FRONTEND_LAYERS.map((layer) => [
    layer,
    new Set(membership[layer].map(normalizeSourcePath)),
  ]));
}

export function classifyFrontendSources(
  sources: readonly ArchitectureSource[],
  literalMembership: FrontendLiteralPolicyMembership = FRONTEND_LITERAL_POLICY_MEMBERSHIP,
): FrontendClassificationReport {
  const literals = literalSets(literalMembership);
  const overridden = new Set([...literals.values()].flatMap((paths) => [...paths]));
  const normalizedSources = [...new Set(sources.map(({ path }) => normalizeSourcePath(path)))].sort();
  const memberships = new Map(FRONTEND_LAYERS.map((layer) => [layer, new Set<string>()]));

  for (const sourceFile of normalizedSources) {
    const owner = baseLayer(sourceFile);
    if (owner !== null && !overridden.has(sourceFile)) memberships.get(owner)!.add(sourceFile);
    for (const layer of FRONTEND_LAYERS) {
      if (literals.get(layer)!.has(sourceFile)) memberships.get(layer)!.add(sourceFile);
    }
  }

  const classification = new Map<string, FrontendLayer>();
  const errors: FrontendClassificationError[] = [];
  for (const sourceFile of normalizedSources) {
    const layers = FRONTEND_LAYERS.filter((layer) => memberships.get(layer)!.has(sourceFile));
    if (layers.length === 0) {
      errors.push({ kind: 'unclassified-production-source', sourceFile });
    } else if (layers.length > 1) {
      errors.push({ kind: 'multiply-classified-production-source', sourceFile, layers });
    } else {
      classification.set(sourceFile, layers[0]);
    }
  }
  return { classification, errors };
}
