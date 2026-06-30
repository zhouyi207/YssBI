import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { PinService } from '@/services';
import { dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { dataValueToBackend } from '@/shared/types/dto/dataValue';
import { dataTypeFromPinType } from '@/shared/types/domain/dataType';
import type { CommandHandler } from '../types';

export interface SetPinValueArgs {
  pinId: string;
  nodeId: string;
  pinType: string;
  newValue: unknown;
}

export interface SetPinValueContext {
  pinId: string;
  nodeId: string;
  pinType: string;
  oldValue: unknown;
  newValue: unknown;
}

export const setPinValueCommand: CommandHandler<SetPinValueArgs, SetPinValueContext> = {
  async execute(graphId, args) {
    const store = useGraphDataStore.getState();
    const pin = store.getGraphPin(graphId, args.pinId);
    const oldValue = pin?.userValue ?? null;

    const dataType = dataTypeFromPinType(args.pinType);
    const dv = dataValueFromRaw(args.newValue, dataType);
    const dto = dataValueToBackend(dv);

    await PinService.updatePinUserValue(graphId, args.nodeId, args.pinId, dto);
    useGraphDataStore.getState().updatePin(args.pinId, { userValue: args.newValue }, graphId);

    return {
      pinId: args.pinId,
      nodeId: args.nodeId,
      pinType: args.pinType,
      oldValue,
      newValue: args.newValue,
    };
  },

  async undo(graphId, context) {
    if (context.oldValue === null || context.oldValue === undefined) {
      await PinService.clearPinUserValue(graphId, context.nodeId, context.pinId);
      useGraphDataStore.getState().updatePin(context.pinId, { userValue: undefined }, graphId);
    } else {
      const dataType = dataTypeFromPinType(context.pinType);
      const dv = dataValueFromRaw(context.oldValue, dataType);
      const dto = dataValueToBackend(dv);
      await PinService.updatePinUserValue(graphId, context.nodeId, context.pinId, dto);
      useGraphDataStore.getState().updatePin(context.pinId, { userValue: context.oldValue }, graphId);
    }
  },

  async redo(graphId, context) {
    const dataType = dataTypeFromPinType(context.pinType);
    const dv = dataValueFromRaw(context.newValue, dataType);
    const dto = dataValueToBackend(dv);
    await PinService.updatePinUserValue(graphId, context.nodeId, context.pinId, dto);
    useGraphDataStore.getState().updatePin(context.pinId, { userValue: context.newValue }, graphId);
  },
};
