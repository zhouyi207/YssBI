import type { PinDirection } from '@/shared/types/domain/pin';
import type { GraphPosition } from '@/shared/types/domain/graph';
import type { GraphDataLike, PinData } from '@/shared/types/store/graph';

export interface MakeTestGraphOptions {
  id: string;
  name?: string;
  type?: 'event' | 'function';
  canvas?: GraphPosition;
  /** Node `title`; defaults to `name` or `id` */
  title?: string;
  nodeId?: string;
  nodeType?: string;
  position?: { x: number; y: number };
  inputPinId?: string;
  outputPinId?: string;
  outputPinColor?: string;
  pinType?: string;
  /** Attach deprecated `links` on pins (hydrate must strip). */
  withLegacyPinLinks?: boolean;
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
  withLegacyPinLinks?: boolean;
}): PinData {
  const pin: PinData & { links?: string[] } = {
    id: params.id,
    nodeId: params.nodeId,
    name: params.name ?? (params.direction === 'input' ? 'In' : 'Out'),
    type: params.type ?? 'Float64',
    direction: params.direction,
    ui: params.ui,
  };
  if (params.withLegacyPinLinks) {
    pin.links = ['should-be-ignored'];
  }
  return pin;
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
      ? { id: optionsOrId, name: legacyTitle ?? optionsOrId, title: legacyTitle }
      : optionsOrId;

  const id = options.id;
  const name = options.name ?? id;
  const nodeId = options.nodeId ?? 'local-node';
  const nodeTitle = options.title ?? name;
  const inputPinId = options.inputPinId ?? 'local-in';
  const outputPinId = options.outputPinId ?? 'local-out';
  const connected = options.connected !== false;

  return {
    id,
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
        withLegacyPinLinks: options.withLegacyPinLinks,
      }),
      makeTestPinData({
        id: outputPinId,
        nodeId,
        direction: 'output',
        type: options.pinType,
        ui: options.outputPinColor ? { color: options.outputPinColor } : undefined,
        withLegacyPinLinks: options.withLegacyPinLinks,
      }),
    ],
    connections: connected
      ? [{ id: `${outputPinId}->${inputPinId}`, from: outputPinId, to: inputPinId }]
      : [],
  };
}

/** Two graphs sharing local node/pin ids (multi-graph isolation tests). */
export function makeOverlappingLocalIdGraphPair(
  first: { id: string; title: string },
  second: { id: string; title: string },
): Record<string, GraphDataLike> {
  return {
    [first.id]: makeTestGraph({ id: first.id, title: first.title }),
    [second.id]: makeTestGraph({ id: second.id, title: second.title }),
  };
}
