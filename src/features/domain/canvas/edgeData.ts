export interface EdgeData {
  readonly id: string;
  readonly fromPinId: string;
  readonly toPinId: string;
  readonly sourceNodeId: string;
  readonly targetNodeId?: string;
  readonly colorKey: string;
  readonly edgeKind: "exec" | "data";
}
