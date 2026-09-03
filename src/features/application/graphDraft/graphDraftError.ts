import { isApplicationIpcError } from "@/features/application/errorReference";

export const GRAPH_DRAFT_ERROR_CODES = [
  "graph_port_not_found",
  "graph_node_not_found",
  "graph_connection_not_found",
  "graph_port_orphan",
  "graph_connection_direction_mismatch",
  "graph_connection_type_mismatch",
  "graph_connection_type_unavailable",
  "graph_connection_type_unresolved",
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

export type GraphDraftErrorCode = (typeof GRAPH_DRAFT_ERROR_CODES)[number];
export type GraphDraftRejectionCode = GraphDraftErrorCode;

type GraphDraftErrorMessageKey = `canvas.connection.errors.${GraphDraftErrorCode}`;

const ERROR_MESSAGE_KEYS: Record<GraphDraftErrorCode, GraphDraftErrorMessageKey> =
  Object.fromEntries(
    GRAPH_DRAFT_ERROR_CODES.map((code) => [code, `canvas.connection.errors.${code}`]),
  ) as Record<GraphDraftErrorCode, GraphDraftErrorMessageKey>;

export function graphDraftErrorCode(error: unknown): GraphDraftErrorCode | null {
  if (!isApplicationIpcError(error)) return null;
  return error.code in ERROR_MESSAGE_KEYS ? (error.code as GraphDraftErrorCode) : null;
}

export function graphDraftErrorMessageKey(code: string): GraphDraftErrorMessageKey | null {
  return code in ERROR_MESSAGE_KEYS ? ERROR_MESSAGE_KEYS[code as GraphDraftErrorCode] : null;
}
