import { IpcError } from '@/services/ipc';

export const GRAPH_MUTATION_ERROR_CODES = [
  'graph_port_not_found',
  'graph_node_not_found',
  'graph_connection_not_found',
  'graph_port_orphan',
  'graph_connection_direction_mismatch',
  'graph_connection_kind_mismatch',
  'graph_connection_type_mismatch',
  'graph_connection_type_unavailable',
  'graph_connection_type_unresolved',
  'graph_connection_limit_reached',
  'graph_connection_order_required',
  'graph_connection_order_forbidden',
  'graph_connection_already_exists',
  'graph_connection_move_source_empty',
  'graph_connection_move_same_port',
  'graph_mutation_empty_targets',
  'graph_mutation_duplicate_target',
  'graph_managed_node_delete_forbidden',
  'graph_revision_conflict',
] as const;

export type GraphMutationErrorCode = typeof GRAPH_MUTATION_ERROR_CODES[number];
export type GraphMutationRejectionCode = Exclude<
  GraphMutationErrorCode,
  'graph_revision_conflict'
>;

type GraphMutationErrorMessageKey =
  `canvas.connection.errors.${GraphMutationErrorCode}`;

const ERROR_MESSAGE_KEYS: Record<GraphMutationErrorCode, GraphMutationErrorMessageKey> =
  Object.fromEntries(GRAPH_MUTATION_ERROR_CODES.map((code) => [
    code,
    `canvas.connection.errors.${code}`,
  ])) as Record<GraphMutationErrorCode, GraphMutationErrorMessageKey>;

export function graphMutationErrorCode(error: unknown): GraphMutationErrorCode | null {
  if (!(error instanceof IpcError)) return null;
  return error.code in ERROR_MESSAGE_KEYS
    ? error.code as GraphMutationErrorCode
    : null;
}

export function graphMutationErrorMessageKey(
  code: string,
): GraphMutationErrorMessageKey | null {
  return code in ERROR_MESSAGE_KEYS
    ? ERROR_MESSAGE_KEYS[code as GraphMutationErrorCode]
    : null;
}
