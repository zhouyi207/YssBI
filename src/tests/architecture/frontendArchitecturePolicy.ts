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

export interface FrontendBaseRule {
  readonly layer: FrontendLayer;
  readonly matches: (path: string) => boolean;
}

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

function baseSetsFromRules(
  rules: readonly FrontendBaseRule[],
  sources: readonly string[],
): MutableMembershipSets {
  const memberships = emptyMembershipSets();
  for (const path of sources) {
    for (const rule of rules) {
      if (rule.matches(path)) memberships.get(rule.layer)!.add(path);
    }
  }
  return memberships;
}

function isSharedPresentationSource(path: string): boolean {
  return /^src\/shared\/(?:ui|charts|hooks|plot)\//.test(path);
}

function isSharedWireSource(path: string): boolean {
  return path.startsWith('src/shared/types/dto/');
}

const PRODUCTION_LITERAL_OVERRIDE_PATHS = new Set(
  FRONTEND_LAYERS.flatMap((layer) => FRONTEND_LITERAL_POLICY_MEMBERSHIP[layer]),
);

export const FRONTEND_BASE_RULES: readonly FrontendBaseRule[] = [
  { layer: 'app-composition', matches: (path) => path.startsWith('src/app/') },
  { layer: 'views', matches: (path) => path.startsWith('src/views/') },
  { layer: 'application', matches: (path) => path.startsWith('src/features/application/') },
  { layer: 'core', matches: (path) => path.startsWith('src/features/core/') },
  { layer: 'domain', matches: (path) => path.startsWith('src/features/domain/') },
  { layer: 'services', matches: (path) => path.startsWith('src/services/') },
  {
    layer: 'components-ui',
    matches: (path) => path.startsWith('src/components/') || isSharedPresentationSource(path),
  },
  { layer: 'wire-schema', matches: isSharedWireSource },
  { layer: 'diagnostics', matches: (path) => path.startsWith('src/utils/') },
  {
    layer: 'pure-shared',
    matches: (path) => path.startsWith('src/lib/')
      || (path.startsWith('src/shared/')
        && !isSharedPresentationSource(path)
        && !isSharedWireSource(path)
        && !PRODUCTION_LITERAL_OVERRIDE_PATHS.has(path)),
  },
];

export function classifyFrontendSources(
  sources: readonly ArchitectureSource[],
  literalMembership: FrontendLiteralPolicyMembership = FRONTEND_LITERAL_POLICY_MEMBERSHIP,
  baseRules: readonly FrontendBaseRule[] = FRONTEND_BASE_RULES,
): FrontendClassificationReport {
  const literals = literalSets(literalMembership);
  const overridden = new Set([...literals.values()].flatMap((paths) => [...paths]));
  const normalizedSources = [...new Set(sources.map(({ path }) => normalizeSourcePath(path)))].sort();
  const sourceSet = new Set(normalizedSources);
  const memberships = baseSetsFromRules(baseRules, normalizedSources);

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
