import type { ArchitectureSource } from '@/tests/helpers/moduleDependencyAudit';
import type {
  FrontendArchitecturePolicy,
  FrontendBasePolicyMembership,
  FrontendClassificationError,
  FrontendClassificationReport,
  FrontendLayer,
  FrontendLiteralPolicyMembership,
} from './frontendArchitectureModel';

export type {
  FrontendBasePolicyMembership,
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

type MutableMembershipSets = Map<FrontendLayer, Set<string>>;

function emptyMembershipSets(): MutableMembershipSets {
  return new Map(FRONTEND_LAYERS.map((layer) => [layer, new Set<string>()]));
}

function literalSets(
  membership: FrontendLiteralPolicyMembership,
): MutableMembershipSets {
  return new Map(FRONTEND_LAYERS.map((layer) => [
    layer,
    new Set(membership[layer].map(normalizeSourcePath)),
  ]));
}

function injectedBaseSets(
  membership: FrontendBasePolicyMembership,
  sources: ReadonlySet<string>,
): MutableMembershipSets {
  const memberships = literalSets(membership);
  for (const paths of memberships.values()) {
    for (const path of paths) {
      if (!sources.has(path)) paths.delete(path);
    }
  }
  return memberships;
}

function productionBaseSets(
  sources: readonly string[],
  overridden: ReadonlySet<string>,
): MutableMembershipSets {
  const memberships = emptyMembershipSets();
  for (const path of sources) {
    const sharedPresentation = /^src\/shared\/(?:ui|charts|hooks|plot)\//.test(path);
    const sharedWire = path.startsWith('src/shared/types/dto/');
    if (path.startsWith('src/app/')) memberships.get('app-composition')!.add(path);
    if (path.startsWith('src/views/')) memberships.get('views')!.add(path);
    if (path.startsWith('src/features/application/')) memberships.get('application')!.add(path);
    if (path.startsWith('src/features/core/')) memberships.get('core')!.add(path);
    if (path.startsWith('src/features/domain/')) memberships.get('domain')!.add(path);
    if (path.startsWith('src/services/')) memberships.get('services')!.add(path);
    if (path.startsWith('src/components/') || sharedPresentation) {
      memberships.get('components-ui')!.add(path);
    }
    if (sharedWire) memberships.get('wire-schema')!.add(path);
    if (path.startsWith('src/utils/')) memberships.get('diagnostics')!.add(path);
    if (path.startsWith('src/lib/')
      || (path.startsWith('src/shared/')
        && !sharedPresentation
        && !sharedWire
        && !overridden.has(path))) {
      memberships.get('pure-shared')!.add(path);
    }
  }
  return memberships;
}

export function classifyFrontendSources(
  sources: readonly ArchitectureSource[],
  literalMembership: FrontendLiteralPolicyMembership = FRONTEND_LITERAL_POLICY_MEMBERSHIP,
  baseMembership?: FrontendBasePolicyMembership,
): FrontendClassificationReport {
  const literals = literalSets(literalMembership);
  const overridden = new Set([...literals.values()].flatMap((paths) => [...paths]));
  const normalizedSources = [...new Set(sources.map(({ path }) => normalizeSourcePath(path)))].sort();
  const sourceSet = new Set(normalizedSources);
  const memberships = baseMembership
    ? injectedBaseSets(baseMembership, sourceSet)
    : productionBaseSets(normalizedSources, overridden);

  for (const layer of FRONTEND_LAYERS) {
    const layerMembership = memberships.get(layer)!;
    for (const path of overridden) layerMembership.delete(path);
    for (const path of literals.get(layer)!) {
      if (sourceSet.has(path)) layerMembership.add(path);
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
