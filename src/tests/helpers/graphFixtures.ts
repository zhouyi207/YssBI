import type { PinDirection } from '@/shared/types/domain/pin';
import type { GraphPosition } from '@/shared/types/domain/graph';
import type { GraphDataLike, PinData } from '@/shared/types/store/graph';

export interface MakeTestGraphOptions {
  path: string;
  name?: string;
  type?: 'event' | 'function';
  canvas?: GraphPosition;
  /** Node `title`; defaults to `name` or `path` */
  title?: string;
  nodeId?: string;
  nodeType?: string;
  position?: { x: number; y: number };
  inputPinId?: string;
  outputPinId?: string;
  outputPinColor?: string;
  pinType?: string;
  /** Default true: `outputPin` → `inputPin` */
  connected?: boolean;
}

function makeTestPinData(params: {
  id: string;
  nodeId: string;
  direction: PinDirection;
  name?: string;
  type?: string;
  ui?: PinData['ui'];
}): PinData {
  return {
    id: params.id,
    nodeId: params.nodeId,
    name: params.name ?? (params.direction === 'input' ? 'In' : 'Out'),
    type: params.type ?? 'Float64',
    direction: params.direction,
    ui: params.ui,
  };
}

/** Hydrate-safe `GraphDataLike` factory for store / canvas / sync tests. */
export function makeTestGraph(options: MakeTestGraphOptions): GraphDataLike;
export function makeTestGraph(id: string, title?: string): GraphDataLike;
export function makeTestGraph(
  optionsOrId: MakeTestGraphOptions | string,
  legacyTitle?: string,
): GraphDataLike {
  const options: MakeTestGraphOptions =
    typeof optionsOrId === 'string'
      ? { path: optionsOrId, name: legacyTitle ?? optionsOrId, title: legacyTitle }
      : optionsOrId;

  const path = options.path;
  const name = options.name ?? path;
  const nodeId = options.nodeId ?? 'local-node';
  const nodeTitle = options.title ?? name;
  const inputPinId = options.inputPinId ?? 'local-in';
  const outputPinId = options.outputPinId ?? 'local-out';
  const connected = options.connected !== false;

  return {
    path,
    name,
    type: options.type ?? 'event',
    canvas: options.canvas ?? { x: 0, y: 0, scale: 1 },
    nodes: [
      {
        id: nodeId,
        nodeType: options.nodeType ?? 'Data:Constant',
        category: ['Data'],
        title: nodeTitle,
        position: options.position ?? { x: 0, y: 0 },
        inputs: [inputPinId],
        outputs: [outputPinId],
      },
    ],
    pins: [
      makeTestPinData({
        id: inputPinId,
        nodeId,
        direction: 'input',
        type: options.pinType,
      }),
      makeTestPinData({
        id: outputPinId,
        nodeId,
        direction: 'output',
        type: options.pinType,
        ui: options.outputPinColor ? { color: options.outputPinColor } : undefined,
      }),
    ],
    connections: connected
      ? [{ id: `${outputPinId}->${inputPinId}`, from: outputPinId, to: inputPinId }]
      : [],
  };
}

/** Two graphs sharing local node/pin ids (multi-graph isolation tests). */
export function makeOverlappingLocalIdGraphPair(
  first: { path: string; title: string },
  second: { path: string; title: string },
): Record<string, GraphDataLike> {
  return {
    [first.path]: makeTestGraph({ path: first.path, title: first.title }),
    [second.path]: makeTestGraph({ path: second.path, title: second.title }),
  };
}
