import type { GraphMutationResultDto } from '@/shared/types/domain/editorMutation';

export type GraphMutationCommandResult =
  | { status: 'applied'; result: GraphMutationResultDto }
  | { status: 'noop'; result: GraphMutationResultDto }
  | { status: 'stale'; result?: GraphMutationResultDto }
  | { status: 'conflict' }
  | { status: 'rejected'; code: string }
  | false;

export type CommandType =
  | 'MoveNodes'
  | 'SetPinValue'
  | 'ConnectPins'
  | 'DisconnectPort'
  | 'DisconnectNode'
  | 'DisconnectConnections'
  | 'InsertReroute'
  | 'MoveConnections'
  | 'DeleteNodes'
  | 'DuplicateSubgraph'
  | 'InsertSubgraph'
  | 'AddRepeatablePin'
  | 'RemoveRepeatablePin';

export interface CommandHandler<TArgs = unknown, TResult = unknown> {
  execute(graphPath: string, args: TArgs): Promise<TResult> | TResult;
}
