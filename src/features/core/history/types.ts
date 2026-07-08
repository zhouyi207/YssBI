/**
 * Command-based Undo/Redo System — Core Types
 *
 * Each editor action is a Command with an execute/undo/redo lifecycle.
 * Commands store only the minimal inverse context needed for undo,
 * not full graph snapshots.
 */

export type CommandType =
  | 'MoveNodes'
  | 'SetPinValue'
  | 'ConnectPins'
  | 'DisconnectPin'
  | 'CreateNode'
  | 'CreateNodeWithConnection'
  | 'DeleteNodes'
  | 'Composite'
  | 'AddRepeatablePin'
  | 'RemoveRepeatablePin';

/**
 * Each CommandHandler encapsulates one editor action.
 * TArgs = arguments for the forward operation.
 * TContext = data captured during execute, needed for undo/redo.
 */
export interface CommandHandler<TArgs = unknown, TContext = unknown> {
  /** Execute the forward operation; return context for undo/redo */
  execute(graphPath: string, args: TArgs): Promise<TContext> | TContext;
  /** Reverse the operation using saved context */
  undo(graphPath: string, context: TContext): Promise<void>;
  /** Re-apply after undo (may differ from initial execute) */
  redo(graphPath: string, context: TContext): Promise<void>;
}

/** A single entry in the undo/redo stack */
export interface HistoryEntry {
  id: string;
  graphPath: string;
  commandType: CommandType;
  /** Command-specific inverse data (opaque to the store) */
  context: unknown;
  timestamp: number;
  mergeKey?: string;
}

/** Per-graph undo/redo stacks */
export interface GraphHistory {
  undoStack: HistoryEntry[];
  redoStack: HistoryEntry[];
}

/** Options passed to executeCommand() */
export interface ExecuteOptions {
  mergeKey?: string;
}
