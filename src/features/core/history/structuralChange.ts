import { useExecutionStore } from '@/features/core/execution';
import type { CommandType } from './types';

const STRUCTURAL_COMMANDS: ReadonlySet<CommandType> = new Set([
  'CreateNode', 'CreateNodeWithConnection', 'DeleteNodes', 'ConnectPins', 'DisconnectPin', 'Composite',
  'AddRepeatablePin', 'RemoveRepeatablePin',
]);

export function notifyStructuralChange(type: CommandType , graphPath: string) {
  if (STRUCTURAL_COMMANDS.has(type)) {
    useExecutionStore.getState().markGraphDirty(graphPath);
  }
}
