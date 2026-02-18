// src/features/core/sync/handlers/VariableEventHandler.ts

import { BaseEventHandler } from './BaseEventHandler';
import {
  VariableCreatedPayload,
  VariableUpdatedPayload,
  VariableDeletedPayload,
  EventCallbacks,
} from '../types';
import { useVariableStore, useGraphDataStore } from '@/features/core/dataStore';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import { dataTypeDisplay } from '@/shared/types/domain/dataType';

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

    // 更新所有引用该变量的节点的 title 和 variableType
    const graphStore = useGraphDataStore.getState();
    const allNodes = graphStore.nodes;
    for (const [nodeId, node] of Object.entries(allNodes)) {
      if (node.variableId === payload.variableId) {
        const prefix = node.nodeType === 'set_variable' ? 'Set ' : 'Get ';
        graphStore.updateNode(nodeId, {
          title: prefix + variable.name,
          variableName: variable.name,
          variableType: dataTypeDisplay(variable.dataType),
        });
      }
    }

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
