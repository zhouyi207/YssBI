import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { CommandHandler } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface SetPinValueArgs {
  pinId: string;
  nodeId: string;
  newValue: unknown;
}

export const setPinValueCommand: CommandHandler<SetPinValueArgs> = {
  execute(graphPath, args) {
    const pin = useGraphDataStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin?.address) throw new Error(`Port '${args.pinId}' has no structured address`);
    if (pin.address.nodeId !== args.nodeId) {
      throw new Error(`Port '${args.pinId}' does not belong to node '${args.nodeId}'`);
    }
    return executeGraphIntent(graphPath, {
      type: 'setLiteral',
      payload: { address: pin.address, literal: args.newValue ?? null },
    });
  },
};
