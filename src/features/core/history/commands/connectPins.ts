import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { CommandHandler, GraphMutationCommandResult } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface ConnectPinsArgs {
  pinA: string;
  pinB: string;
}

export const connectPinsCommand: CommandHandler<ConnectPinsArgs, GraphMutationCommandResult> = {
  execute(graphPath, args) {
    const store = useGraphDataStore.getState();
    const pinA = store.getGraphPin(graphPath, args.pinA);
    const pinB = store.getGraphPin(graphPath, args.pinB);
    if (!pinA?.address || !pinB?.address) {
      throw new Error('Cannot connect ports without structured projection addresses');
    }
    const output = pinA.direction === 'output' ? pinA : pinB;
    const input = pinA.direction === 'input' ? pinA : pinB;
    if (output.direction !== 'output' || input.direction !== 'input') {
      throw new Error('A connection requires one output port and one input port');
    }
    return executeGraphIntent(graphPath, {
      type: 'connect',
      payload: { output: output.address!, input: input.address!, order: null },
    });
  },
};
