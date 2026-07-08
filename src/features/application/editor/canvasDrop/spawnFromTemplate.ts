import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';
import type { NodeSpawnTemplate } from '@/features/core/dnd';
import type { EditorFunctions, EditorVariables } from '@/features/core/editor';
import { logger } from '@/utils/appLogger';
import type { CreateNodeFn } from './createNodeFn';
import { isFunctionAvailable, isVariableAvailable } from './editorResources';
import {
  buildVariableDropMenu,
  resolveVariableSpawnType,
  spawnVariableNode,
  type VariableDropMenu,
  type VariableNodeType,
} from './variableDrop';

export interface SpawnFromTemplateContext {
  variables: EditorVariables;
  functions: EditorFunctions;
  createNode: CreateNodeFn;
  onVariableMenu: (menu: VariableDropMenu) => void;
}

export async function spawnNodeFromTemplate(
  template: NodeSpawnTemplate,
  worldPosition: { x: number; y: number },
  clientPosition: { x: number; y: number },
  event: Pick<MouseEvent | PointerEvent, 'altKey' | 'ctrlKey'>,
  ctx: SpawnFromTemplateContext,
): Promise<void> {
  if (template.category === 'Data') {
    if (!template.variableId) {
      logger.graph.warn('DataFrame drop missing variableId', 'CanvasDrop');
      return;
    }
    await ctx.createNode(template.nodeType, worldPosition, {
      dataframeId: template.variableId,
      variableName: template.variableName,
    });
    return;
  }

  if (template.category === 'Variable') {
    if (!template.variableId) {
      logger.graph.warn('Variable drop missing variableId', 'CanvasDrop');
      return;
    }
    if (!isVariableAvailable(template.variableId, ctx.variables)) {
      logger.graph.warn('Variable no longer exists. Aborting drop', 'CanvasDrop');
      return;
    }

    const spawnType = resolveVariableSpawnType(event, clientPosition.x, clientPosition.y);
    if (spawnType === 'menu') {
      ctx.onVariableMenu(buildVariableDropMenu(
        clientPosition.x,
        clientPosition.y,
        worldPosition,
        template.variableId,
        template.variableName ?? template.title ?? template.variableId,
      ));
      return;
    }

    await spawnVariableNode(spawnType, worldPosition, template.variableId, ctx.createNode);
    return;
  }

  if (template.nodeType === CALL_FUNCTION_NODE_TYPE) {
    if (!template.subGraphId) {
      logger.graph.warn('Function call drop missing subGraphId', 'CanvasDrop');
      return;
    }
    if (!isFunctionAvailable(template.subGraphId, ctx.functions)) return;
    await ctx.createNode(CALL_FUNCTION_NODE_TYPE, worldPosition, { subGraphId: template.subGraphId });
    return;
  }

  await ctx.createNode(template.nodeType, worldPosition);
}

export type { VariableNodeType };
