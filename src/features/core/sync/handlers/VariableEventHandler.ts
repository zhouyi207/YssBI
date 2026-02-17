// src/features/core/sync/handlers/VariableEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import {
  VariableCreatedPayload,
  VariableUpdatedPayload,
  VariableDeletedPayload,
  EventCallbacks,
} from '../types';
import { useVariableStore } from '@/features/core/dataStore';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';

export class VariableCreatedHandler extends BaseEventHandler<VariableCreatedPayload> {
  eventType = 'VariableCreated';

  handle(payload: VariableCreatedPayload, callbacks?: EventCallbacks): void {
    this.log('Variable created:', payload.variableId);

    const variable = normalizeVariableFromBackend(payload.data);
    useVariableStore.getState().addVariable(payload.variableId, variable);

    callbacks?.onVariableCreated?.(payload.variableId, variable);
  }
}

export class VariableUpdatedHandler extends BaseEventHandler<VariableUpdatedPayload> {
  eventType = 'VariableUpdated';

  handle(payload: VariableUpdatedPayload, callbacks?: EventCallbacks): void {
    this.log('Variable updated:', payload.variableId);

    const variable = normalizeVariableFromBackend(payload.data);
    useVariableStore.getState().updateVariable(payload.variableId, variable);

    callbacks?.onVariableUpdated?.(payload.variableId, variable);
  }
}

export class VariableDeletedHandler extends BaseEventHandler<VariableDeletedPayload> {
  eventType = 'VariableDeleted';

  handle(payload: VariableDeletedPayload, callbacks?: EventCallbacks): void {
    this.log('Variable deleted:', payload.variableId);

    useVariableStore.getState().deleteVariable(payload.variableId);

    callbacks?.onVariableDeleted?.(payload.variableId);
  }
}
