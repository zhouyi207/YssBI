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
import { rebuildVariableResourceProjection } from '@/features/application/dataManagement/variableActions';
import { markVariableScopeDirty } from '@/features/core/variable/variableDirty';

export class VariableCreatedHandler extends BaseEventHandler<VariableCreatedPayload> {
  eventType = 'VariableCreated';

  handle(payload: VariableCreatedPayload, callbacks?: EventCallbacks): void {
    this.log('Variable created:', payload.variableId);

    const variable = normalizeVariableFromBackend(payload.data);
    useVariableStore.getState().addVariable(payload.variableId, variable);
    rebuildVariableResourceProjection();
    markVariableScopeDirty(payload.variableScope);

    callbacks?.onVariableCreated?.(payload.variableId, variable);
  }
}

export class VariableUpdatedHandler extends BaseEventHandler<VariableUpdatedPayload> {
  eventType = 'VariableUpdated';

  handle(payload: VariableUpdatedPayload, callbacks?: EventCallbacks): void {
    this.log('Variable updated:', payload.variableId);

    const variable = normalizeVariableFromBackend(payload.data);
    useVariableStore.getState().updateVariable(payload.variableId, variable);
    rebuildVariableResourceProjection();
    markVariableScopeDirty(payload.variableScope);

    callbacks?.onVariableUpdated?.(payload.variableId, variable);
  }
}

export class VariableDeletedHandler extends BaseEventHandler<VariableDeletedPayload> {
  eventType = 'VariableDeleted';

  handle(payload: VariableDeletedPayload, callbacks?: EventCallbacks): void {
    this.log('Variable deleted:', payload.variableId);

    useVariableStore.getState().deleteVariable(payload.variableId);
    rebuildVariableResourceProjection();
    markVariableScopeDirty(payload.variableScope);

    callbacks?.onVariableDeleted?.(payload.variableId);
  }
}
