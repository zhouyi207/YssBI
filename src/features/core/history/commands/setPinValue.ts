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

function resolvePinDataType(graphPath: string, pinId: string) {
  const pin = useGraphDataStore.getState().getGraphPin(graphPath, pinId);
  if (!pin) throw new Error(`Pin ${pinId} not found`);
  if (isExecPin(pin)) throw new Error(`Cannot set value on exec pin ${pinId}`);
  return buildPinDataType(pin as Pin);
}

export const setPinValueCommand: CommandHandler<SetPinValueArgs, SetPinValueContext> = {
  async execute(graphPath, args) {
    const store = useGraphDataStore.getState();
    const pin = store.getGraphPin(graphPath, args.pinId);
    const oldValue = pin?.userValue ?? null;

    const dataType = resolvePinDataType(graphPath, args.pinId);
    const dv = dataValueFromRaw(args.newValue, dataType);
    const dto = dataValueToBackend(dv);

    await PinService.updatePinUserValue(graphPath, args.nodeId, args.pinId, dto);
    useGraphDataStore.getState().updatePin(args.pinId, { userValue: args.newValue }, graphPath);

    return {
      pinId: args.pinId,
      nodeId: args.nodeId,
      oldValue,
      newValue: args.newValue,
    };
  },

  async undo(graphPath, context) {
    if (context.oldValue === null || context.oldValue === undefined) {
      await PinService.clearPinUserValue(graphPath, context.nodeId, context.pinId);
      useGraphDataStore.getState().updatePin(context.pinId, { userValue: undefined }, graphPath);
    } else {
      const dataType = resolvePinDataType(graphPath, context.pinId);
      const dv = dataValueFromRaw(context.oldValue, dataType);
      const dto = dataValueToBackend(dv);
      await PinService.updatePinUserValue(graphPath, context.nodeId, context.pinId, dto);
      useGraphDataStore.getState().updatePin(context.pinId, { userValue: context.oldValue }, graphPath);
    }
  },

  async redo(graphPath, context) {
    const dataType = resolvePinDataType(graphPath, context.pinId);
    const dv = dataValueFromRaw(context.newValue, dataType);
    const dto = dataValueToBackend(dv);
    await PinService.updatePinUserValue(graphPath, context.nodeId, context.pinId, dto);
    useGraphDataStore.getState().updatePin(context.pinId, { userValue: context.newValue }, graphPath);
  },
};
