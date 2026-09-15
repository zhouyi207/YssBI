import { isApplicationIpcError } from "@/features/application/errorReference";

export const GRAPH_EDIT_ERROR_CODES = [
  "graph_edit_changed",
  "graph_edit_busy",
  "graph_port_not_found",
  "graph_node_not_found",
  "graph_connection_not_found",
  "graph_port_orphan",
  "graph_connection_direction_mismatch",
  "graph_connection_type_mismatch",
  "graph_connection_type_unavailable",
  "graph_connection_limit_reached",
  "graph_connection_order_required",
  "graph_connection_order_forbidden",
  "graph_connection_already_exists",
  "graph_connection_move_source_empty",
  "graph_connection_move_same_port",
  "graph_mutation_empty_targets",
  "graph_mutation_duplicate_target",
  "graph_managed_node_delete_forbidden",
] as const;

export type GraphEditErrorCode = (typeof GRAPH_EDIT_ERROR_CODES)[number];
export type GraphEditRejectionCode = GraphEditErrorCode;

type GraphEditErrorMessageKey = `canvas.connection.errors.${GraphEditErrorCode}`;

const ERROR_MESSAGE_KEYS: Record<GraphEditErrorCode, GraphEditErrorMessageKey> = Object.fromEntries(
  GRAPH_EDIT_ERROR_CODES.map((code) => [code, `canvas.connection.errors.${code}`]),
) as Record<GraphEditErrorCode, GraphEditErrorMessageKey>;

export function graphEditErrorCode(error: unknown): GraphEditErrorCode | null {
  if (!isApplicationIpcError(error)) return null;
  return error.code in ERROR_MESSAGE_KEYS ? (error.code as GraphEditErrorCode) : null;
}

export function graphEditErrorMessageKey(code: string): GraphEditErrorMessageKey | null {
  return code in ERROR_MESSAGE_KEYS ? ERROR_MESSAGE_KEYS[code as GraphEditErrorCode] : null;
}
