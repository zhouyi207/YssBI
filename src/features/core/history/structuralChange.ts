import { useExecutionStore } from '@/features/core/execution';
import type { CommandType } from './types';

const STRUCTURAL_COMMANDS: ReadonlySet<CommandType> = new Set([
  'CreateNode', 'DeleteNodes', 'ConnectPins', 'DisconnectPin', 'Composite',
]);

export function notifyStructuralChange(type: CommandType, graphId: string) {
  if (STRUCTURAL_COMMANDS.has(type)) {
    useExecutionStore.getState().markGraphDirty(graphId);
  }
}
