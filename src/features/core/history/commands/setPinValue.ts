import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { PinService } from '@/services';
import { dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { dataValueToBackend } from '@/shared/types/dto/dataValue';
import type { Pin } from '@/shared/types/domain/pin';
import { isExecPin } from '@/shared/types/domain/pinSemantics';
import { buildPinDataType } from '@/shared/utils/pinCompatibility';
import type { CommandHandler } from '../types';

export interface SetPinValueArgs {
  pinId: string;
  nodeId: string;
  newValue: unknown;
}

export interface SetPinValueContext {
  pinId: string;
  nodeId: string;
  oldValue: unknown;
  newValue: unknown;
}

function resolvePinDataType(graphId: string, pinId: string) {
  const pin = useGraphDataStore.getState().getGraphPin(graphId, pinId);
  if (!pin) throw new Error(`Pin ${pinId} not found`);
  if (isExecPin(pin)) throw new Error(`Cannot set value on exec pin ${pinId}`);
  return buildPinDataType(pin as Pin);
}

export const setPinValueCommand: CommandHandler<SetPinValueArgs, SetPinValueContext> = {
  async execute(graphId, args) {
    const store = useGraphDataStore.getState();
    const pin = store.getGraphPin(graphId, args.pinId);
    const oldValue = pin?.userValue ?? null;

    const dataType = resolvePinDataType(graphId, args.pinId);
    const dv = dataValueFromRaw(args.newValue, dataType);
    const dto = dataValueToBackend(dv);

    await PinService.updatePinUserValue(graphId, args.nodeId, args.pinId, dto);
    useGraphDataStore.getState().updatePin(args.pinId, { userValue: args.newValue }, graphId);

    return {
      pinId: args.pinId,
      nodeId: args.nodeId,
      oldValue,
      newValue: args.newValue,
    };
  },

  async undo(graphId, context) {
    if (context.oldValue === null || context.oldValue === undefined) {
      await PinService.clearPinUserValue(graphId, context.nodeId, context.pinId);
      useGraphDataStore.getState().updatePin(context.pinId, { userValue: undefined }, graphId);
    } else {
      const dataType = resolvePinDataType(graphId, context.pinId);
      const dv = dataValueFromRaw(context.oldValue, dataType);
      const dto = dataValueToBackend(dv);
      await PinService.updatePinUserValue(graphId, context.nodeId, context.pinId, dto);
      useGraphDataStore.getState().updatePin(context.pinId, { userValue: context.oldValue }, graphId);
    }
  },

  async redo(graphId, context) {
    const dataType = resolvePinDataType(graphId, context.pinId);
    const dv = dataValueFromRaw(context.newValue, dataType);
    const dto = dataValueToBackend(dv);
    await PinService.updatePinUserValue(graphId, context.nodeId, context.pinId, dto);
    useGraphDataStore.getState().updatePin(context.pinId, { userValue: context.newValue }, graphId);
  },
};
