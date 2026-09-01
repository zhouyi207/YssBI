import type { PinDirection } from "@/shared/types/domain/pin";
import type { DataType } from "@/shared/types/domain/dataType";
import type { GraphData, PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";

export interface MakeTestGraphOptions {
  path: string;
  name?: string;
  type?: "event" | "function";
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

function defaultDataType(type?: string): DataType | undefined {
  if (!type || type === "exec") return undefined;
  if (type === "Float64") return { kind: "Float64" };
  if (type === "Int64") return { kind: "Int64" };
  return { kind: "Any" };
}

function makeTestPinData(params: {
  id: string;
  nodeId: string;
  direction: PinDirection;
  name?: string;
  type?: string;
  dataType?: DataType;
  ui?: PinData["ui"];
}): PinData {
  const pinType = params.type ?? "Float64";
  const isExec = pinType === "exec";
  const dataType = params.dataType ?? defaultDataType(pinType);
  return {
    id: params.id,
    nodeId: params.nodeId,
    name: params.name ?? (params.direction === "input" ? "In" : "Out"),
    type: isExec ? "exec" : "object",
    direction: params.direction,
    dataType,
    ui: params.ui,
  };
}

/** Canonical `GraphData` factory for store / canvas / sync tests. */
export function makeTestGraph(options: MakeTestGraphOptions): GraphData {
  const path = options.path;
  const name = options.name ?? path;
  const nodeId = options.nodeId ?? "local-node";
  const nodeTitle = options.title ?? name;
  const inputPinId = options.inputPinId ?? "local-in";
  const outputPinId = options.outputPinId ?? "local-out";
  const connected = options.connected !== false;

  return {
    path,
    name,
    type: options.type ?? "event",
    nodes: [
      {
        id: nodeId,
        graphPath: path,
        nodeType: options.nodeType ?? "Data:Constant",
        category: ["Data"],
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
        direction: "input",
        type: options.pinType,
      }),
      makeTestPinData({
        id: outputPinId,
        nodeId,
        direction: "output",
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
): Record<string, GraphData> {
  return {
    [first.path]: makeTestGraph({ path: first.path, title: first.title }),
    [second.path]: makeTestGraph({ path: second.path, title: second.title }),
  };
}
