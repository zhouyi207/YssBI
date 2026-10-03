import { freezePublishedValue } from "@/shared/types/deepReadonly";
import { produce, type Draft } from "immer";
import type {
  GraphPath,
  ConnectionData,
  ConnectionId,
  NodeData,
  NodeId,
  PinData,
  PinId,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import type {
  EditorGraphProjectionDto,
  GraphConstantDto,
  GraphEditorSessionDto,
  GraphEditVersionDto,
} from "@/shared/types/domain/editorMutation";
import type {
  EditorNodeProjectionDto,
  EditorPortDto,
  ParameterEditorDto,
  ParameterGroupDto,
  ParameterEditorSpecDto,
  EditorConnectionProjectionDto,
  DiagnosticDto,
  DiagnosticLocationDto,
  ProjectionBasisDto,
  PortTypeStateDto,
  SchemaSummaryDto,
} from "@/shared/types/domain/editorProjection";
import type { GraphResultState } from "@/shared/types/domain/result";
import { portAddressKey } from "@/features/domain/editorProjection";
import { validateEditorGraphProjection } from "@/shared/types/domain/editorProjectionParser";
import { shareProjection } from "@/features/core/state/readProjection";
import { shareGraphResultState } from "./graphResultProjection";
import { shareGraphConstants } from "./graphConstantProjection";
import { restoreValueType, restoreValueTypes } from "./valueTypeProjection";
import type { GraphEntityBucket } from "./graphEntityAccess";

export type { GraphEntityBucket } from "./graphEntityAccess";

/** A read mirror of the Rust editing session; the backend owns document/history writes. */
export interface GraphEditorState {
  readonly constants: Record<string, GraphConstantDto> | undefined;
  readonly projection: EditorGraphProjectionDto;
  readonly version: GraphEditVersionDto;
  readonly sessionId: number;
  readonly projectionGeneration: number;
  readonly semanticInputHash: string;
  readonly saveDirty: boolean;
  readonly saving: boolean;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
}

export interface GraphProjectionData {
  graphEntities: Record<GraphPath, GraphEntityBucket>;
  sessions: Readonly<Record<GraphPath, GraphEditorState>>;
  resultStates: Readonly<Record<GraphPath, GraphResultState>>;
}

export function canAcceptGraphSession(
  previous: GraphEditorState | undefined,
  input: GraphEditorSessionDto,
): boolean {
  return (
    previous?.version.sessionId !== input.editing.version.sessionId ||
    BigInt(previous.version.revision) <= BigInt(input.editing.version.revision)
  );
}

function sameFields<T extends object>(previous: T | undefined, next: T): previous is T {
  if (!previous) return false;
  const keys = Object.keys(next) as Array<keyof T>;
  return (
    keys.length === Object.keys(previous).length &&
    keys.every(
      (key) =>
        Object.prototype.hasOwnProperty.call(previous, key) && Object.is(previous[key], next[key]),
    )
  );
}

function shareItems<T>(previous: T[] | undefined, next: T[]): T[] {
  return previous?.length === next.length && next.every((id, i) => id === previous[i])
    ? previous
    : next;
}

function restoreConnections(
  previous: EditorConnectionProjectionDto[],
  next: EditorConnectionProjectionDto[],
  draft: () => Draft<EditorConnectionProjectionDto[]>,
): boolean {
  if (previous === next) return true;
  let equal = previous.length === next.length;
  for (let index = 0; index < next.length; index++) {
    const connection = next[index];
    const before = previous[index];
    if (before === connection) continue;
    if (!before) {
      equal = false;
      continue;
    }
    const output = sameFields(before.output, connection.output) ? before.output : connection.output;
    const input = sameFields(before.input, connection.input) ? before.input : connection.input;
    if (
      before.connectionId === connection.connectionId &&
      before.order === connection.order &&
      before.output === output &&
      before.input === input
    ) {
      draft()[index] = before;
    } else {
      equal = false;
      if (output !== connection.output) draft()[index].output = output;
      if (input !== connection.input) draft()[index].input = input;
    }
  }
  return equal;
}

function shareDiagnosticLocation(
  previous: DiagnosticLocationDto | undefined,
  next: DiagnosticLocationDto,
): DiagnosticLocationDto {
  if (
    previous?.kind === "port" &&
    next.kind === "port" &&
    sameFields(previous.address, next.address)
  )
    return previous;
  return sameFields(previous, next) ? previous : next;
}

function restoreDiagnostics(
  previous: DiagnosticDto[],
  next: DiagnosticDto[],
  draft: () => Draft<DiagnosticDto[]>,
): boolean {
  if (previous === next) return true;
  let equal = previous.length === next.length;
  for (let index = 0; index < next.length; index++) {
    const diagnostic = next[index];
    const before = previous[index];
    if (before === diagnostic) continue;
    if (!before) {
      equal = false;
      continue;
    }
    const argumentsValue = sameFields(before.arguments, diagnostic.arguments)
      ? before.arguments
      : diagnostic.arguments;
    const location = shareDiagnosticLocation(before.location, diagnostic.location);
    let relatedEqual = before.related === diagnostic.related;
    if (!relatedEqual) {
      relatedEqual = before.related.length === diagnostic.related.length;
      for (let relatedIndex = 0; relatedIndex < diagnostic.related.length; relatedIndex++) {
        const nextLocation = diagnostic.related[relatedIndex];
        const shared = shareDiagnosticLocation(before.related[relatedIndex], nextLocation);
        if (shared !== before.related[relatedIndex]) relatedEqual = false;
        if (shared !== nextLocation) draft()[index].related[relatedIndex] = shared;
      }
    }
    if (
      before.code === diagnostic.code &&
      before.messageKey === diagnostic.messageKey &&
      before.severity === diagnostic.severity &&
      before.blocking === diagnostic.blocking &&
      before.arguments === argumentsValue &&
      before.location === location &&
      relatedEqual
    ) {
      draft()[index] = before;
    } else {
      equal = false;
      if (argumentsValue !== diagnostic.arguments) draft()[index].arguments = argumentsValue;
      if (location !== diagnostic.location) draft()[index].location = location;
      if (relatedEqual && before.related !== diagnostic.related)
        draft()[index].related = before.related;
    }
  }
  return equal;
}

function restoreBasis(
  previous: ProjectionBasisDto,
  next: ProjectionBasisDto,
  draft: () => Draft<ProjectionBasisDto>,
): boolean {
  if (previous === next) return true;
  const versions = sameFields(previous.resourceVersions, next.resourceVersions)
    ? previous.resourceVersions
    : next.resourceVersions;
  const observations = shareResourceObservations(
    previous.resourceObservations,
    next.resourceObservations,
  );
  if (
    previous.graphPath === next.graphPath &&
    previous.registryFingerprint === next.registryFingerprint &&
    previous.semanticInputHash === next.semanticInputHash &&
    previous.resourceVersions === versions &&
    previous.resourceObservations === observations
  )
    return true;
  if (versions !== next.resourceVersions) draft().resourceVersions = versions;
  if (observations !== next.resourceObservations) draft().resourceObservations = observations;
  return false;
}

function shareResourceObservations(
  previous: ProjectionBasisDto["resourceObservations"],
  next: ProjectionBasisDto["resourceObservations"],
): ProjectionBasisDto["resourceObservations"] {
  if (
    previous === next ||
    Object.getPrototypeOf(previous) !== Object.prototype ||
    Object.getPrototypeOf(next) !== Object.prototype
  )
    return next;
  const keys = Object.keys(next);
  let equal = keys.length === Object.keys(previous).length;
  let shared = next;
  for (const key of keys) {
    const before = Object.prototype.hasOwnProperty.call(previous, key) ? previous[key] : undefined;
    if (!sameFields(before, next[key])) {
      equal = false;
    } else if (before !== next[key]) {
      // Spread creates own data properties, including __proto__; these are not draft paths.
      if (shared === next) shared = { ...next };
      shared[key] = before;
    }
  }
  return equal ? previous : shared;
}

function restoreTypeState(
  previous: PortTypeStateDto,
  next: PortTypeStateDto,
  draft: () => Draft<PortTypeStateDto>,
): boolean {
  if (previous === next) return true;
  if (previous.status === "exact" && next.status === "exact") {
    const dataTypeEqual = restoreValueType(previous.dataType, next.dataType, () => {
      const target = draft() as Draft<typeof next>;
      return target.dataType!;
    });
    if (previous.display === next.display && dataTypeEqual) return true;
    if (dataTypeEqual && previous.dataType !== next.dataType) {
      const target = draft();
      if (target.status === "exact") target.dataType = previous.dataType;
    }
    return false;
  }
  if (previous.status === "constrained" && next.status === "constrained") {
    const domainEqual = restoreValueTypes(previous.domain, next.domain, () => {
      const target = draft() as Draft<typeof next>;
      return target.domain;
    });
    if (previous.display === next.display && domainEqual) return true;
    if (domainEqual && previous.domain !== next.domain) {
      const target = draft();
      if (target.status === "constrained") target.domain = previous.domain;
    }
    return false;
  }
  return sameFields(previous, next);
}

function restoreSchema(
  previous: SchemaSummaryDto | null,
  next: SchemaSummaryDto | null,
  draft: () => Draft<SchemaSummaryDto>,
): boolean {
  if (previous === next) return true;
  if (!previous || !next) return false;
  let fieldsEqual = previous.fields === next.fields;
  if (!fieldsEqual) {
    fieldsEqual = previous.fields.length === next.fields.length;
    for (let index = 0; index < next.fields.length; index++) {
      const field = next.fields[index];
      const before = previous.fields[index];
      if (sameFields(before, field)) {
        if (before !== field) draft().fields[index] = before;
      } else fieldsEqual = false;
    }
  }
  if (previous.kind === next.kind && fieldsEqual) return true;
  if (fieldsEqual && previous.fields !== next.fields) draft().fields = previous.fields;
  return false;
}

function restoreParameterEditor(
  previous: ParameterEditorSpecDto,
  next: ParameterEditorSpecDto,
  draft: () => Draft<ParameterEditorSpecDto>,
): boolean {
  if (previous === next) return true;
  if (previous.kind === "select" && next.kind === "select")
    return (
      previous.options === next.options ||
      (previous.options !== null &&
        next.options !== null &&
        sameFields(previous.options, next.options))
    );
  if (previous.kind === "projectColumns" && next.kind === "projectColumns") {
    const target = () => draft() as Draft<typeof next>;
    let optionsEqual = previous.options === next.options;
    if (!optionsEqual) {
      optionsEqual = previous.options.length === next.options.length;
      for (let index = 0; index < next.options.length; index++) {
        const option = next.options[index];
        const before = previous.options[index];
        if (sameFields(before, option)) {
          if (before !== option) target().options[index] = before;
        } else optionsEqual = false;
      }
    }
    const valueEqual = sameFields(previous.value, next.value);
    if (
      previous.allowEmpty === next.allowEmpty &&
      previous.available === next.available &&
      previous.unavailableReason === next.unavailableReason &&
      optionsEqual &&
      valueEqual
    )
      return true;
    if (optionsEqual && previous.options !== next.options) target().options = previous.options;
    if (valueEqual && previous.value !== next.value) target().value = previous.value;
    return false;
  }
  if (previous.kind === "filterPredicate" && next.kind === "filterPredicate") {
    const target = () => draft() as Draft<typeof next>;
    let columnsEqual = previous.columns === next.columns;
    if (!columnsEqual) {
      columnsEqual = previous.columns.length === next.columns.length;
      for (let index = 0; index < next.columns.length; index++) {
        const column = next.columns[index];
        const before = previous.columns[index];
        if (!before) {
          columnsEqual = false;
          continue;
        }
        if (before === column) continue;
        const operatorsEqual = sameFields(before.operators, column.operators);
        const literalsEqual = sameFields(before.literalTypes, column.literalTypes);
        if (
          before.name === column.name &&
          before.dataType === column.dataType &&
          operatorsEqual &&
          literalsEqual
        )
          target().columns[index] = before;
        else {
          columnsEqual = false;
          if (operatorsEqual && before.operators !== column.operators)
            target().columns[index].operators = before.operators;
          if (literalsEqual && before.literalTypes !== column.literalTypes)
            target().columns[index].literalTypes = before.literalTypes;
        }
      }
    }
    let valueEqual = previous.value === next.value;
    if (!valueEqual && previous.value && next.value) {
      const literal =
        next.value.value && sameFields(previous.value.value, next.value.value)
          ? previous.value.value
          : next.value.value;
      valueEqual =
        previous.value.column === next.value.column &&
        previous.value.operator === next.value.operator &&
        previous.value.value === literal;
      if (!valueEqual && literal !== next.value.value) target().value!.value = literal;
    }
    if (
      previous.available === next.available &&
      previous.unavailableReason === next.unavailableReason &&
      columnsEqual &&
      valueEqual
    )
      return true;
    if (columnsEqual && previous.columns !== next.columns) target().columns = previous.columns;
    if (valueEqual && previous.value !== next.value) target().value = previous.value;
    return false;
  }
  return sameFields(previous, next);
}

/** Restore fixed parameter shells in the graph's single Immer transaction. */
function restoreParameterGroups(
  previous: ParameterGroupDto[],
  next: ParameterGroupDto[],
  draft: () => Draft<ParameterGroupDto[]>,
): boolean {
  if (previous === next) return true;
  let equal = previous.length === next.length;
  let groups: Map<string, ParameterGroupDto> | undefined;
  for (let index = 0; index < next.length; index++) {
    const group = next[index];
    const atIndex = previous[index];
    const before =
      atIndex?.key === group.key
        ? atIndex
        : (groups ??= new Map(previous.map((entry) => [entry.key, entry]))).get(group.key);
    if (!before) {
      equal = false;
      continue;
    }
    if (before !== atIndex) equal = false;
    if (before === group) continue;
    const display = sameFields(before.display, group.display) ? before.display : group.display;
    let parametersEqual = before.parameters === group.parameters;
    if (!parametersEqual) {
      parametersEqual = before.parameters.length === group.parameters.length;
      let parameters: Map<string, ParameterEditorDto> | undefined;
      for (let parameterIndex = 0; parameterIndex < group.parameters.length; parameterIndex++) {
        const parameter = group.parameters[parameterIndex];
        const atParameterIndex = before.parameters[parameterIndex];
        const previousParameter =
          atParameterIndex?.key === parameter.key
            ? atParameterIndex
            : (parameters ??= new Map(before.parameters.map((entry) => [entry.key, entry]))).get(
                parameter.key,
              );
        if (!previousParameter) {
          parametersEqual = false;
          continue;
        }
        if (previousParameter !== atParameterIndex) parametersEqual = false;
        if (previousParameter === parameter) continue;
        const parameterDisplay = sameFields(previousParameter.display, parameter.display)
          ? previousParameter.display
          : parameter.display;
        const target = () => draft()[index].parameters[parameterIndex];
        const editorEqual = restoreParameterEditor(
          previousParameter.editor,
          parameter.editor,
          () => target().editor,
        );
        const valueTypeEqual = restoreValueType(
          previousParameter.valueType,
          parameter.valueType,
          () => target().valueType!,
        );
        const value = shareProjection(previousParameter.value, parameter.value);
        if (
          previousParameter.display === parameterDisplay &&
          editorEqual &&
          previousParameter.presentation === parameter.presentation &&
          valueTypeEqual &&
          previousParameter.multiline === parameter.multiline &&
          Object.is(previousParameter.value, value)
        ) {
          draft()[index].parameters[parameterIndex] = previousParameter;
        } else {
          parametersEqual = false;
          if (parameterDisplay !== parameter.display) target().display = parameterDisplay;
          if (editorEqual && previousParameter.editor !== parameter.editor)
            target().editor = previousParameter.editor;
          if (valueTypeEqual && previousParameter.valueType !== parameter.valueType)
            target().valueType = previousParameter.valueType;
          if (!Object.is(value, parameter.value)) target().value = value;
        }
      }
    }
    if (before.display === display && parametersEqual) {
      draft()[index] = before;
    } else {
      equal = false;
      if (display !== group.display) draft()[index].display = display;
      if (parametersEqual && before.parameters !== group.parameters)
        draft()[index].parameters = before.parameters;
    }
  }
  return equal;
}

function restorePort(
  previous: EditorPortDto,
  next: EditorPortDto,
  draft: () => Draft<EditorPortDto>,
): boolean {
  if (previous === next) return true;
  const address = sameFields(previous.address, next.address) ? previous.address : next.address;
  const display = sameFields(previous.display, next.display) ? previous.display : next.display;
  const connections = sameFields(previous.connections, next.connections)
    ? previous.connections
    : next.connections;
  const typeStateEqual = restoreTypeState(
    previous.typeState,
    next.typeState,
    () => draft().typeState,
  );
  const schemaEqual = restoreSchema(
    previous.resolvedSchema,
    next.resolvedSchema,
    () => draft().resolvedSchema!,
  );
  let inputEqual = previous.input === next.input;
  if (!inputEqual && previous.input && next.input) {
    // JSON values are compared outside the draft. Their keys never become draft paths.
    const literalOverride = shareProjection(
      previous.input.literalOverride,
      next.input.literalOverride,
    );
    const protocolDefault = shareProjection(
      previous.input.protocolDefault,
      next.input.protocolDefault,
    );
    inputEqual =
      previous.input.effective === next.input.effective &&
      Object.is(previous.input.literalOverride, literalOverride) &&
      Object.is(previous.input.protocolDefault, protocolDefault);
    if (!inputEqual) {
      if (!Object.is(literalOverride, next.input.literalOverride))
        draft().input!.literalOverride = literalOverride;
      if (!Object.is(protocolDefault, next.input.protocolDefault))
        draft().input!.protocolDefault = protocolDefault;
    }
  }
  const equal =
    previous.address === address &&
    previous.display === display &&
    previous.direction === next.direction &&
    previous.orphan === next.orphan &&
    previous.canRemove === next.canRemove &&
    previous.connections === connections &&
    inputEqual &&
    typeStateEqual &&
    schemaEqual &&
    previous.status === next.status;
  if (equal) return true;
  if (address !== next.address) draft().address = address;
  if (display !== next.display) draft().display = display;
  if (connections !== next.connections) draft().connections = connections;
  if (inputEqual && previous.input !== next.input) draft().input = previous.input;
  if (typeStateEqual && previous.typeState !== next.typeState)
    draft().typeState = previous.typeState;
  if (schemaEqual && previous.resolvedSchema !== next.resolvedSchema)
    draft().resolvedSchema = previous.resolvedSchema;
  return false;
}

function restoreNode(
  previous: EditorNodeProjectionDto,
  next: EditorNodeProjectionDto,
  draft: () => Draft<EditorNodeProjectionDto>,
): boolean {
  if (previous === next) return true;
  const position = sameFields(previous.position, next.position) ? previous.position : next.position;
  const display = sameFields(previous.display, next.display) ? previous.display : next.display;
  const capabilities = sameFields(previous.capabilities, next.capabilities)
    ? previous.capabilities
    : next.capabilities;
  let additionsEqual = previous.portInstanceAdditions === next.portInstanceAdditions;
  if (!additionsEqual) {
    additionsEqual = previous.portInstanceAdditions.length === next.portInstanceAdditions.length;
    for (let index = 0; index < next.portInstanceAdditions.length; index++) {
      const addition = next.portInstanceAdditions[index];
      const before = previous.portInstanceAdditions[index];
      if (sameFields(before, addition)) {
        if (before !== addition) draft().portInstanceAdditions[index] = before;
      } else additionsEqual = false;
    }
  }
  const diagnosticsEqual = restoreDiagnostics(
    previous.diagnostics,
    next.diagnostics,
    () => draft().diagnostics,
  );
  const groupsEqual = restoreParameterGroups(
    previous.parameterGroups,
    next.parameterGroups,
    () => draft().parameterGroups,
  );
  let portsEqual = previous.ports === next.ports;
  if (!portsEqual) {
    portsEqual = previous.ports.length === next.ports.length;
    let ports: Map<PinId, EditorPortDto> | undefined;
    for (let index = 0; index < next.ports.length; index++) {
      const port = next.ports[index];
      const atIndex = previous.ports[index];
      const key = portAddressKey(port.address);
      const before =
        atIndex && portAddressKey(atIndex.address) === key
          ? atIndex
          : (ports ??= new Map(
              previous.ports.map((entry) => [portAddressKey(entry.address), entry]),
            )).get(key);
      if (!before) {
        portsEqual = false;
        continue;
      }
      if (before !== atIndex) portsEqual = false;
      if (restorePort(before, port, () => draft().ports[index])) {
        if (before !== port) draft().ports[index] = before;
      } else portsEqual = false;
    }
  }
  const equal =
    previous.graphPath === next.graphPath &&
    previous.nodeId === next.nodeId &&
    previous.nodeTypeId === next.nodeTypeId &&
    previous.position === position &&
    previous.display === display &&
    previous.capabilities === capabilities &&
    additionsEqual &&
    diagnosticsEqual &&
    groupsEqual &&
    portsEqual;
  if (equal) return true;
  if (position !== next.position) draft().position = position;
  if (display !== next.display) draft().display = display;
  if (capabilities !== next.capabilities) draft().capabilities = capabilities;
  if (additionsEqual && previous.portInstanceAdditions !== next.portInstanceAdditions)
    draft().portInstanceAdditions = previous.portInstanceAdditions;
  if (diagnosticsEqual && previous.diagnostics !== next.diagnostics)
    draft().diagnostics = previous.diagnostics;
  if (groupsEqual && previous.parameterGroups !== next.parameterGroups)
    draft().parameterGroups = previous.parameterGroups;
  if (portsEqual && previous.ports !== next.ports) draft().ports = previous.ports;
  return false;
}

/** One transaction owns typed topology; external read-value branches retain their boundary sharing. */
function shareGraphProjection(
  previous: EditorGraphProjectionDto | undefined,
  next: EditorGraphProjectionDto,
): EditorGraphProjectionDto {
  if (!previous || previous === next) return next;
  const outcome = sameFields(previous.outcome, next.outcome) ? previous.outcome : next.outcome;
  let basisEqual = false;
  let connectionsEqual = false;
  let diagnosticsEqual = false;
  let nodesEqual = previous.nodes === next.nodes;
  const shared = produce(next, (draft) => {
    basisEqual = restoreBasis(previous.basis, next.basis, () => draft.basis);
    connectionsEqual = restoreConnections(
      previous.connections,
      next.connections,
      () => draft.connections,
    );
    diagnosticsEqual = restoreDiagnostics(
      previous.diagnostics,
      next.diagnostics,
      () => draft.diagnostics,
    );
    if (!nodesEqual) {
      nodesEqual = previous.nodes.length === next.nodes.length;
      let nodes: Map<NodeId, EditorNodeProjectionDto> | undefined;
      for (let index = 0; index < next.nodes.length; index++) {
        const node = next.nodes[index];
        const atIndex = previous.nodes[index];
        const before =
          atIndex?.nodeId === node.nodeId
            ? atIndex
            : (nodes ??= new Map(previous.nodes.map((entry) => [entry.nodeId, entry]))).get(
                node.nodeId,
              );
        if (!before) {
          nodesEqual = false;
          continue;
        }
        if (before !== atIndex) nodesEqual = false;
        if (restoreNode(before, node, () => draft.nodes[index])) {
          if (before !== node) draft.nodes[index] = before;
        } else nodesEqual = false;
      }
    }
    if (nodesEqual && previous.nodes !== next.nodes) draft.nodes = previous.nodes;
    if (basisEqual && previous.basis !== next.basis) draft.basis = previous.basis;
    if (connectionsEqual && previous.connections !== next.connections)
      draft.connections = previous.connections;
    if (diagnosticsEqual && previous.diagnostics !== next.diagnostics)
      draft.diagnostics = previous.diagnostics;
    if (outcome !== next.outcome) draft.outcome = outcome;
  });
  // Whole equality prefers previous; a delta needing no substitutions retains next,
  // including its validation/freeze proofs. Identity matching never changes old order.
  return nodesEqual &&
    basisEqual &&
    previous.graphPath === next.graphPath &&
    connectionsEqual &&
    diagnosticsEqual &&
    previous.outcome === outcome &&
    previous.hasBlockingDiagnostics === next.hasBlockingDiagnostics
    ? previous
    : shared;
}

function buildProjectionBucket(
  graphPath: string,
  projection: EditorGraphProjectionDto,
  previous?: GraphEntityBucket,
  previousProjection?: EditorGraphProjectionDto,
): GraphEntityBucket {
  // Keep the whole-graph identity/endpoint checks before publishing any derived indexes.
  validateEditorGraphProjection(projection);
  if (projection.graphPath !== graphPath)
    throw new Error(`Projection '${projection.graphPath}' does not match '${graphPath}'`);
  const base: GraphEntityBucket = previous ?? {
    basis: projection.basis,
    diagnostics: projection.diagnostics,
    outcome: projection.outcome,
    hasBlockingDiagnostics: projection.hasBlockingDiagnostics,
    nodes: Object.create(null),
    pins: Object.create(null),
    connections: Object.create(null),
    graphNodes: [],
    pinConnections: Object.create(null),
    blockedConnectionIds: Object.create(null),
    primaryPortDiagnostics: Object.create(null),
  };
  const update = (bucket: GraphEntityBucket) => {
    const clearPortDiagnostics = (node: NodeData) => {
      for (const diagnostic of node.diagnostics) {
        const location = diagnostic.location;
        if (location.kind === "port" && location.address.nodeId === node.id)
          delete bucket.primaryPortDiagnostics[portAddressKey(location.address)];
      }
    };
    bucket.basis = projection.basis;
    bucket.diagnostics = projection.diagnostics;
    bucket.outcome = projection.outcome;
    bucket.hasBlockingDiagnostics = projection.hasBlockingDiagnostics;
    if (!previous || projection.diagnostics !== previous.diagnostics) {
      const blocked: Record<ConnectionId, true> = Object.create(null);
      for (const diagnostic of projection.diagnostics) {
        if (diagnostic.blocking && diagnostic.location.kind === "connection")
          blocked[diagnostic.location.connectionId] = true;
      }
      bucket.blockedConnectionIds = sameFields(previous?.blockedConnectionIds, blocked)
        ? previous.blockedConnectionIds
        : blocked;
    }
    // Read the immutable source, not draft dictionaries: untouched entries need no proxies.
    if (!previous || projection.nodes !== previousProjection?.nodes) {
      const ids = shareItems(
        base.graphNodes,
        projection.nodes.map((node) => node.nodeId),
      );
      bucket.graphNodes = ids;
      let previousNodesById: Map<NodeId, EditorNodeProjectionDto> | undefined;
      for (const [index, node] of projection.nodes.entries()) {
        const before = base.nodes[node.nodeId];
        const atIndex = previousProjection?.nodes[index];
        const source =
          atIndex?.nodeId === node.nodeId
            ? atIndex
            : previousProjection &&
              (previousNodesById ??= new Map(
                previousProjection.nodes.map((entry) => [entry.nodeId, entry]),
              )).get(node.nodeId);
        if (before && node === source) continue;
        let pinIds = before?.pinIds ?? [];
        if (!before || node.ports !== source?.ports) {
          pinIds = shareItems(
            before?.pinIds,
            node.ports.map((port) => portAddressKey(port.address)),
          );
          for (const [portIndex, port] of node.ports.entries()) {
            const id = pinIds[portIndex];
            const candidate: PinData = {
              id,
              nodeId: port.address.nodeId,
              name: port.display.instanceLabel ?? port.display.label,
              direction: port.direction,
              address: port.address,
              display: port.display,
              orphan: port.orphan,
              canRemove: port.canRemove,
              connections: port.connections,
              input: port.input,
              typeState: port.typeState,
              resolvedSchema: port.resolvedSchema,
              status: port.status,
            };
            bucket.pins[id] = sameFields(base.pins[id], candidate) ? base.pins[id] : candidate;
            if (!base.pinConnections[id]) bucket.pinConnections[id] = [];
          }
          if (before && pinIds !== before.pinIds) {
            const retained = new Set(pinIds);
            for (const id of before.pinIds) {
              if (retained.has(id)) continue;
              delete bucket.pins[id];
              delete bucket.pinConnections[id];
            }
          }
        }
        const shared: NodeData = {
          id: node.nodeId,
          graphPath: node.graphPath,
          nodeType: node.nodeTypeId,
          position: node.position,
          pinIds,
          display: node.display,
          parameterGroups: node.parameterGroups,
          portInstanceAdditions: node.portInstanceAdditions,
          capabilities: node.capabilities,
          diagnostics: node.diagnostics,
        };
        bucket.nodes[node.nodeId] = sameFields(before, shared) ? before : shared;
        if (!before || shared.diagnostics !== before.diagnostics) {
          // Fresh nodes populate the graph index directly; changed nodes stage only their entries.
          const diagnostics: GraphEntityBucket["primaryPortDiagnostics"] = before
            ? Object.create(null)
            : bucket.primaryPortDiagnostics;
          for (const diagnostic of shared.diagnostics) {
            const location = diagnostic.location;
            if (location.kind !== "port" || location.address.nodeId !== shared.id) continue;
            const id = portAddressKey(location.address);
            const selected = diagnostics[id];
            // Preserve the old per-node lookup: first blocking, otherwise first matching.
            if (!selected || (!selected.blocking && diagnostic.blocking))
              diagnostics[id] = diagnostic;
          }
          if (before) {
            for (const diagnostic of before.diagnostics) {
              const location = diagnostic.location;
              if (location.kind !== "port" || location.address.nodeId !== before.id) continue;
              const id = portAddressKey(location.address);
              if (!diagnostics[id]) delete bucket.primaryPortDiagnostics[id];
            }
            for (const id in diagnostics) {
              if (base.primaryPortDiagnostics[id] !== diagnostics[id])
                bucket.primaryPortDiagnostics[id] = diagnostics[id];
            }
          }
        }
      }
      if (ids !== base.graphNodes) {
        const retained = new Set(ids);
        for (const id of base.graphNodes) {
          if (retained.has(id)) continue;
          for (const pinId of base.nodes[id].pinIds) {
            delete bucket.pins[pinId];
            delete bucket.pinConnections[pinId];
          }
          clearPortDiagnostics(base.nodes[id]);
          delete bucket.nodes[id];
        }
      }
    }
    if (!previous || projection.connections !== previousProjection?.connections) {
      const retained = new Set<ConnectionId>();
      const affectedPorts = new Set<PinId>();
      const connections: ConnectionData[] = [];
      for (const [index, connection] of projection.connections.entries()) {
        const id = connection.connectionId;
        retained.add(id);
        const existing = base.connections[id];
        const atIndex = previousProjection?.connections[index];
        const from =
          existing && connection === atIndex ? existing.from : portAddressKey(connection.output);
        const to =
          existing && connection === atIndex ? existing.to : portAddressKey(connection.input);
        const candidate: ConnectionData = {
          id,
          from,
          to,
          output: existing?.from === from ? existing.output : connection.output,
          input: existing?.to === to ? existing.input : connection.input,
          order: connection.order,
        };
        const shared = sameFields(existing, candidate) ? existing : candidate;
        bucket.connections[id] = shared;
        connections.push(shared);
        if (atIndex?.connectionId !== id || existing?.from !== from || existing?.to !== to) {
          affectedPorts.add(from).add(to);
          if (existing) affectedPorts.add(existing.from).add(existing.to);
        }
      }
      for (const connection of previousProjection?.connections ?? []) {
        if (retained.has(connection.connectionId)) continue;
        const existing = base.connections[connection.connectionId];
        affectedPorts.add(existing.from).add(existing.to);
        delete bucket.connections[connection.connectionId];
      }
      // Only affected adjacency lists are rebuilt. Scan source order to preserve reordering,
      // including duplicate endpoints on a diagnostic-blocked self connection.
      const adjacency = new Map([...affectedPorts].map((id) => [id, [] as ConnectionId[]]));
      for (const connection of connections) {
        adjacency.get(connection.from)?.push(connection.id);
        adjacency.get(connection.to)?.push(connection.id);
      }
      for (const [id, connectionIds] of adjacency) {
        if (bucket.pins[id])
          bucket.pinConnections[id] = shareItems(base.pinConnections[id], connectionIds);
      }
    }
  };
  if (previous) return produce(base, update);
  // A fresh table has no readers yet. Populate it before the existing publication freeze,
  // avoiding both per-entry proxies and a second deep freeze during initial load.
  update(base);
  return base;
}

let nextViewSessionId = 0;

interface PreparedGraphSession {
  session: GraphEditorState;
  bucket: GraphEntityBucket;
  resultState: GraphResultState;
}

function prepareSession(
  state: GraphProjectionData,
  graphPath: string,
  input: GraphEditorSessionDto,
  saving?: boolean,
  renew = false,
): PreparedGraphSession | undefined {
  const previous = state.sessions[graphPath];
  if (!canAcceptGraphSession(previous, input)) return;
  if (input.resultState.semanticInputHash !== input.projection.basis.semanticInputHash) {
    throw new Error("Graph projection and result state must have the same semantic identity");
  }
  const constants = shareGraphConstants(previous?.constants, input.document.constants);
  const projection = shareGraphProjection(previous?.projection, input.projection);
  const currentResults = state.resultStates[graphPath];
  const resultState =
    currentResults &&
    currentResults.executionSessionId === input.resultState.executionSessionId &&
    currentResults.semanticInputHash === input.resultState.semanticInputHash &&
    BigInt(currentResults.revision) > BigInt(input.resultState.revision)
      ? currentResults
      : shareGraphResultState(currentResults, input.resultState);
  const sessionUnchanged =
    !renew &&
    previous &&
    previous.constants === constants &&
    previous.projection === projection &&
    previous.version.sessionId === input.editing.version.sessionId &&
    previous.version.revision === input.editing.version.revision &&
    previous.saveDirty === input.editing.dirty &&
    previous.canUndo === input.editing.canUndo &&
    previous.canRedo === input.editing.canRedo &&
    (saving === undefined || saving === previous.saving);
  if (sessionUnchanged && resultState === state.resultStates[graphPath]) return;
  const bucket =
    projection === previous?.projection && state.graphEntities[graphPath]
      ? state.graphEntities[graphPath]
      : buildProjectionBucket(
          graphPath,
          projection,
          state.graphEntities[graphPath],
          previous?.projection,
        );
  return {
    session: sessionUnchanged
      ? previous
      : {
          constants,
          projection,
          version: sameFields(previous?.version, input.editing.version)
            ? previous.version
            : input.editing.version,
          sessionId:
            !renew && previous?.version.sessionId === input.editing.version.sessionId
              ? previous.sessionId
              : ++nextViewSessionId,
          projectionGeneration: (previous?.projectionGeneration ?? 0) + 1,
          semanticInputHash: projection.basis.semanticInputHash,
          saveDirty: input.editing.dirty,
          canUndo: input.editing.canUndo,
          canRedo: input.editing.canRedo,
          saving: saving ?? previous?.saving ?? false,
        },
    bucket,
    resultState,
  };
}

export interface PreparedGraphSessions {
  readonly graphPaths: readonly string[];
  readonly state: GraphProjectionData;
}

/** Prepare all accepted sessions before a single publication, including project snapshots. */
export function prepareGraphSessions(
  replacements: readonly {
    graphPath: string;
    session: GraphEditorSessionDto;
    saving?: boolean;
    renew?: boolean;
  }[],
  retainedGraphPaths: ReadonlySet<string> | undefined,
  base: GraphProjectionData,
): PreparedGraphSessions {
  const retain = <T>(values: Readonly<Record<string, T>>): Record<string, T> =>
    !retainedGraphPaths || Object.keys(values).every((path) => retainedGraphPaths.has(path))
      ? values
      : Object.fromEntries(Object.entries(values).filter(([path]) => retainedGraphPaths.has(path)));
  const retained: GraphProjectionData = {
    graphEntities: retain(base.graphEntities),
    sessions: retain(base.sessions),
    resultStates: retain(base.resultStates),
  };
  let sessions: Record<string, GraphEditorState> = retained.sessions;
  let graphEntities = retained.graphEntities;
  let resultStates: Record<string, GraphResultState> = retained.resultStates;
  const paths = new Set<string>();
  for (const { graphPath, session, saving, renew } of replacements) {
    if (paths.has(graphPath)) throw new Error(`Duplicate graph session '${graphPath}'`);
    paths.add(graphPath);
    const prepared = prepareSession(retained, graphPath, session, saving, renew);
    if (!prepared) continue;
    // Each project-level table is copied at most once. Tables already filtered by retain
    // are owned by this unpublished batch; entity updates still use their existing owner.
    if (prepared.session !== sessions[graphPath]) {
      if (sessions === base.sessions) sessions = { ...sessions };
      sessions[graphPath] = prepared.session;
    }
    if (prepared.bucket !== graphEntities[graphPath]) {
      if (graphEntities === base.graphEntities) graphEntities = { ...graphEntities };
      graphEntities[graphPath] = prepared.bucket;
    }
    if (prepared.resultState !== resultStates[graphPath]) {
      if (resultStates === base.resultStates) resultStates = { ...resultStates };
      resultStates[graphPath] = prepared.resultState;
    }
  }
  const state =
    sessions === base.sessions &&
    graphEntities === base.graphEntities &&
    resultStates === base.resultStates
      ? base
      : { sessions, graphEntities, resultStates };
  if (state !== base) freezePublishedValue(state);
  return { graphPaths: [...paths], state };
}
