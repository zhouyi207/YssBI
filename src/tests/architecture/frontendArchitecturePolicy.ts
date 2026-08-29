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
  core: [],
  domain: [],
  services: [
    'src/shared/platform/tauriWebview.ts',
    'src/shared/utils/openExternalUrl.ts',
  ],
  'components-ui': [
    'src/shared/theme/dockviewTheme.ts',
  ],
  'wire-schema': [],
  diagnostics: [],
  'pure-shared': [],
};

const LAYER_EDGES = [
  ['app-composition', 'views'],
  ['app-composition', 'application'],
  ['app-composition', 'services'],
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
  ['application', 'diagnostics'],
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

function viewCoreCapability(
  canonicalModule: string,
  exportedSymbols: readonly string[],
  memberCapabilities: Readonly<Record<string, readonly string[]>> | null = null,
) {
  return {
    sourceLayer: 'views' as const,
    canonicalModule,
    exportedSymbols,
    exactConsumers: null,
    memberCapabilities,
  };
}

const VIEW_CORE_CAPABILITIES = [
  viewCoreCapability('src/features/core/canvas/connectionInteraction.ts', ['ConnectionFeedback']),
  viewCoreCapability('src/features/core/canvas/connectPreview.ts', [
    'getConnectPreview',
    'subscribeConnectPreview',
  ]),
  viewCoreCapability('src/features/core/canvas/edgePath.ts', ['computeEdgePath']),
  viewCoreCapability('src/features/core/canvas/useEdgeDragPreview.ts', ['useEdgeDragPreview']),
  viewCoreCapability('src/features/core/canvas/useNodeDragPreview.ts', ['useNodeDragPreview']),
  viewCoreCapability('src/features/core/canvas/useSelectionBoxPreview.ts', ['useSelectionBoxPreview']),
  viewCoreCapability('src/features/core/database/read.ts', ['useDatabaseRead']),
  viewCoreCapability('src/features/core/dataStore/graphEntityAccess.ts', ['GraphEntitiesState']),
  viewCoreCapability('src/features/core/dataStore/nodeView.ts', [
    'uiNodeHasNoHeader',
    'uiNodeIsReroute',
  ]),
  viewCoreCapability('src/features/core/dataStore/pinLinks.ts', ['derivePinConnectionView']),
  viewCoreCapability('src/features/core/dataStore/useNodeView.ts', ['useNodeView']),
  viewCoreCapability('src/features/core/dockview/logsDockviewLayout.ts', [
    'DEFAULT_LOGS_DOCKVIEW_LAYOUT',
    'LOGS_DOCKVIEW_COMPONENT_ID',
    'LogsDockviewPanelParams',
  ]),
  viewCoreCapability('src/features/core/dockview/workbenchDockviewDefaults.ts', [
    'WORKBENCH_ACTIVITY_GROUP_ID',
  ]),
  viewCoreCapability('src/features/core/dockview/workbenchPanelModel.ts', [
    'EditorResourceKind',
    'isWorkbenchActivityViewId',
    'isWorkbenchPersistentViewMetadata',
    'layoutTabFromEditorMetadata',
    'WorkbenchComponentId',
    'WorkbenchPanelMetadata',
    'WorkbenchPanelParams',
    'WorkbenchViewId',
  ]),
  viewCoreCapability('src/features/core/editor/ui.ts', ['editorUi'], {
    editorUi: [
      'getSnapshot',
      'subscribe',
      'setContextMenu',
      'setDetailFocus',
      'clearDetailFocus',
      'setVariablesGraphScope',
    ],
  }),
  viewCoreCapability('src/features/core/editor/ui.ts', ['useEditorUi']),
  viewCoreCapability('src/features/core/execution/executionVisualSession.ts', [
    'connectionKey',
    'getExecutionVisual',
    'subscribeExecutionVisual',
  ]),
  viewCoreCapability('src/features/core/execution/graphRunArtifacts.ts', ['graphHasClearableArtifacts']),
  viewCoreCapability('src/features/core/execution/pinResultIndex.ts', ['pinHistoryCacheKey']),
  viewCoreCapability('src/features/core/execution/pinResultSearch.ts', ['PinResultSearchEntry']),
  viewCoreCapability('src/features/core/execution/pinViewTarget.ts', [
    'buildPinViewParams',
    'evaluatePinViewState',
    'pinViewDisabledTitle',
  ]),
  viewCoreCapability('src/features/core/execution/read.ts', ['useExecutionRead']),
  viewCoreCapability('src/features/core/execution/ui.ts', ['executionResultUi'], {
    executionResultUi: ['recordPinHistory', 'getPinHistory', 'clearRunOutput'],
  }),
  viewCoreCapability('src/features/core/execution/useExecutionPlayback.ts', ['useExecutionPlayback']),
  viewCoreCapability('src/features/core/execution/useExecutionVisualBinder.ts', ['useExecutionVisualBinder']),
  viewCoreCapability('src/features/core/graph/read.ts', ['useGraphRead']),
  viewCoreCapability('src/features/core/graphInteraction/ui.ts', ['useGraphInteractionUi']),
  viewCoreCapability('src/features/core/graphSession/graphSessionStore.ts', ['FocusedGraphSession']),
  viewCoreCapability('src/features/core/graphSession/ui.ts', ['useGraphSessionUi']),
  viewCoreCapability('src/features/core/history/types.ts', ['GraphMutationCommandResult']),
  viewCoreCapability('src/features/core/keyboard/ui.ts', ['keyboardUi']),
  viewCoreCapability('src/features/core/layout/layoutTabQueries.ts', ['getActiveLayoutTab']),
  viewCoreCapability('src/features/core/node/useNodeExecution.ts', ['useNodeExecution']),
  viewCoreCapability('src/features/core/node/useNodeStyle.ts', ['useNodeStyle']),
  viewCoreCapability('src/features/core/pin/usePinInput.ts', ['usePinInput']),
  viewCoreCapability('src/features/core/pin/useRepeatablePinRemovable.ts', ['useRepeatablePinRemovable']),
  viewCoreCapability('src/features/core/plugins/ui.ts', ['pluginUi'], {
    pluginUi: ['getSnapshot', 'subscribe', 'installPlugin', 'uninstallPlugin'],
  }),
  viewCoreCapability('src/features/core/plugins/ui.ts', ['usePluginUi']),
  viewCoreCapability('src/features/core/resource/functionResourceView.ts', ['FunctionResourceView']),
  viewCoreCapability('src/features/core/resource/read.ts', ['useResourceRead']),
  viewCoreCapability('src/features/core/resource/resourceSelectors.ts', ['GraphResourceRecord']),
  viewCoreCapability('src/features/core/resource/resourceTypes.ts', ['resourceKey']),
  viewCoreCapability('src/features/core/settings/read.ts', ['useSettingsRead']),
  viewCoreCapability('src/features/core/settings/ui.ts', ['settingsUi'], {
    settingsUi: [
      'setTheme',
      'setEditorOption',
      'updateTheme',
      'updateEditor',
      'updateAppearance',
      'updateProject',
      'resetAllToDefaults',
      'resetThemeToDefaults',
      'resetEditorToDefaults',
      'resetAppearanceToDefaults',
      'resetProjectToDefaults',
    ],
  }),
  viewCoreCapability('src/features/core/sidebar/flatRows/buildDataSidebarModel.ts', ['buildDataSidebarModel']),
  viewCoreCapability('src/features/core/sidebar/flatRows/sidebarPanelModel.ts', ['SidebarPanelModel']),
  viewCoreCapability('src/features/core/sidebar/flatRows/types.ts', [
    'SIDEBAR_FLAT_ROW_HEIGHT',
    'SidebarItemRow',
    'SidebarSectionActionConfig',
  ]),
  viewCoreCapability('src/features/core/sidebar/projectTreeState.ts', [
    'PROJECT_TREE_CATEGORY_IDS',
    'ProjectTreeCategoryId',
  ]),
  viewCoreCapability('src/features/core/sidebar/sidebarSectionState.ts', ['SidebarSectionKey']),
  viewCoreCapability('src/features/core/sidebar/ui.ts', ['sidebarUi'], {
    sidebarUi: [
      'getSnapshot',
      'subscribe',
      'toggleSection',
      'setSectionExpanded',
      'setProjectTreeQuery',
      'setProjectTreeCategoryExpanded',
      'setProjectTreeCategoriesExpanded',
      'resetProjectTreeQuery',
    ],
  }),
  viewCoreCapability('src/features/core/sidebar/ui.ts', ['useSidebarUi']),
  viewCoreCapability('src/features/core/sidebarDrag/ui.ts', ['sidebarDragUi'], {
    sidebarDragUi: [
      'getSnapshot',
      'subscribe',
      'setActiveDrag',
      'updatePosition',
      'setCanvasDropHandler',
      'getCanvasDropHandler',
      'subscribeCanvasDropHandlers',
    ],
  }),
  viewCoreCapability('src/features/core/sidebarDrag/ui.ts', ['useSidebarDragUi']),
  viewCoreCapability('src/features/core/statusBar/statusBarItemTypes.ts', ['StatusBarItemViewModel']),
  viewCoreCapability('src/features/core/theme/pinTypeTheme.ts', ['getPinTypeColor']),
  viewCoreCapability('src/features/core/theme/useTheme.ts', ['useTheme']),
  viewCoreCapability('src/features/core/ui/ui.ts', ['ui'], {
    ui: ['confirm', 'confirm3'],
  }),
  viewCoreCapability('src/features/core/variable/read.ts', ['useVariableRead']),
  viewCoreCapability('src/features/core/viewport/viewportScope.ts', [
    'editorViewportScope',
    'ViewportScope',
  ]),
  viewCoreCapability('src/features/core/viewport/viewportSession.ts', [
    'getViewport',
    'subscribeToViewport',
  ]),
  viewCoreCapability('src/features/core/viewport/viewportTransform.ts', [
    'applyViewportGrid',
    'applyViewportTransform',
    'viewportGridStyle',
    'viewportTransformStyle',
  ]),
  viewCoreCapability('src/features/core/workbench/ui.ts', ['workbenchUi'], {
    workbenchUi: [
      'getSnapshot',
      'subscribe',
      'setSettingsOpen',
      'setNodeDocumentationOpen',
      'openSettings',
    ],
  }),
  viewCoreCapability('src/features/core/workbench/ui.ts', ['useWorkbenchUi']),
  viewCoreCapability('src/features/core/worksheet/read.ts', ['useWorksheetRead']),
  viewCoreCapability('src/features/core/worksheet/ui.ts', ['worksheetUi'], {
    worksheetUi: ['updateDraft', 'discardDraft'],
  }),
];

const VIEW_DOMAIN_CAPABILITIES = [
  viewCoreCapability('src/features/domain/bayes/validationFormatting.ts', ['issueTargetStep']),
  viewCoreCapability('src/features/domain/bayes/diagnostics.ts', [
    'diagnosticSeverityClass',
    'DiagnosticSuggestion',
    'evaluateInferenceDiagnostics',
    'parameterDiagnosticStatus',
  ]),
  viewCoreCapability('src/features/domain/bayes/expressionAst.ts', [
    'formatExpression',
    'formatRawExpressionLatex',
  ]),
  viewCoreCapability('src/features/domain/bayes/priorDefaults.ts', [
    'defaultPriorForConstraint',
    'formatPrior',
  ]),
  viewCoreCapability('src/features/domain/canvas/edgeData.ts', ['EdgeData']),
  viewCoreCapability('src/features/domain/graphDiagnostics/nodeDiagnostics.ts', [
    'collectNodeDiagnostics',
    'GraphNodeDiagnostic',
  ]),
  viewCoreCapability('src/features/domain/log/logDomains.ts', ['isLogDomainId', 'LogDomainId']),
  viewCoreCapability('src/features/domain/node/utils/nodeClassNames.ts', [
    'getNodeBackgroundStyle',
    'getNodeClassName',
    'getNodeMinSize',
    'REROUTE_GRIP_SIZE_PX',
    'REROUTE_NODE_HEIGHT_PX',
    'REROUTE_NODE_WIDTH_PX',
  ]),
  viewCoreCapability('src/features/domain/nodeCatalog/catalogItem.ts', [
    'catalogItemKey',
    'LocalizedCatalogItem',
  ]),
  viewCoreCapability('src/features/domain/nodeCatalog/creationDescriptor.ts', ['NodeCreationDescriptor']),
  viewCoreCapability('src/features/domain/nodeCatalog/localizedCatalogTree.ts', [
    'LocalizedCatalogBrowserRow',
  ]),
  viewCoreCapability('src/features/domain/sidebar/constants.ts', [
    'PIN_COLORS',
    'TYPE_ICON_COLORS',
  ]),
];

export const FRONTEND_ARCHITECTURE_POLICY: FrontendArchitecturePolicy = {
  layerEdges: LAYER_EDGES,
  capabilities: [
    ...VIEW_CORE_CAPABILITIES,
    ...VIEW_DOMAIN_CAPABILITIES,
    {
      sourceLayer: 'app-composition',
      canonicalModule: 'src/features/core/dockview/workbenchRead.ts',
      exportedSymbols: ['WorkbenchDockviewRead'],
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
        'buildSidebarDragState',
        'getSidebarDragOverlayLabel',
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
      canonicalModule: 'src/features/core/dockview/workbenchRead.ts',
      exportedSymbols: ['workbenchDockviewRead'],
      exactConsumers: null,
      memberCapabilities: {
        workbenchDockviewRead: WORKBENCH_DOCKVIEW_READ_MEMBERS,
      },
    },
    {
      sourceLayer: 'views',
      canonicalModule: 'src/features/core/dockview/workbenchTypes.ts',
      exportedSymbols: ['WorkbenchPanelInfo'],
      exactConsumers: null,
      memberCapabilities: null,
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
      exportedSymbols: ['logsDockviewRootBinding', 'LogsDockviewBindingToken'],
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
