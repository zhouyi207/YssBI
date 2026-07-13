import type { LayoutNode, LayoutTree } from '@/shared/types/ui';
import {
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
  createInitialWorkbenchNodes,
} from './workbenchLayoutDefaults';
import {
  readEditorAreaMaximizedGroupId,
  readEditorAreaRestoredGridSizes,
  listEditorGroupIds,
  setEditorGroupMaximizedHidden,
} from './editorGridLayout';
import { computeEditorGridMementoSizes } from './editorGridSizing';
import { isEditorGroupNode } from './layoutTabQueries';

export interface EditorGridNodeMemento {
  id: string;
  type: LayoutNode['type'];
  parentId: string | null;
  children?: string[];
  /** Normalized flex weight within the parent split (0–1). */
  size?: number;
}

export interface EditorGridMemento {
  activeEditorGroupId: string;
  maximizedGroupId?: string | null;
  restoredGridSizes?: Record<string, number>;
  nodes: EditorGridNodeMemento[];
}

function isEditorGroupSnapshot(node: EditorGridNodeMemento): boolean {
  return node.type === 'component' && isEditorGroupNode({
    id: node.id,
    type: node.type,
    parentId: node.parentId,
  });
}

function collectEditorAreaNodeIds(nodes: LayoutTree): Set<string> {
  const ids = new Set<string>([EDITOR_AREA_ID]);
  const visit = (id: string) => {
    const node = nodes[id];
    if (!node?.children) return;
    for (const childId of node.children) {
      ids.add(childId);
      visit(childId);
    }
  };
  visit(EDITOR_AREA_ID);
  return ids;
}

export function snapshotEditorGridMemento(
  nodes: LayoutTree,
  activeEditorGroupId: string | null,
): EditorGridMemento | null {
  const editorArea = nodes[EDITOR_AREA_ID];
  if (!editorArea) return null;

  const ids = collectEditorAreaNodeIds(nodes);
  const normalizedSizes = computeEditorGridMementoSizes(nodes);
  const gridNodes: EditorGridNodeMemento[] = [];
  for (const id of ids) {
    const node = nodes[id];
    if (!node) continue;
    gridNodes.push({
      id: node.id,
      type: node.type,
      parentId: node.parentId,
      children: node.children ? [...node.children] : undefined,
      size: normalizedSizes[id] ?? node.size,
    });
  }

  return {
    activeEditorGroupId: activeEditorGroupId ?? DEFAULT_EDITOR_GROUP_ID,
    maximizedGroupId: readEditorAreaMaximizedGroupId(nodes),
    restoredGridSizes: readEditorAreaRestoredGridSizes(nodes) ?? undefined,
    nodes: gridNodes,
  };
}

export function applyEditorGridMemento(
  nodes: LayoutTree,
  memento: EditorGridMemento,
): LayoutTree {
  const next = { ...nodes };
  const existingIds = collectEditorAreaNodeIds(next);
  for (const id of existingIds) {
    if (id !== EDITOR_AREA_ID) delete next[id];
  }

  for (const snapshot of memento.nodes) {
    const existing = next[snapshot.id];
    if (isEditorGroupSnapshot(snapshot)) {
      next[snapshot.id] = {
        id: snapshot.id,
        type: 'component',
        parentId: snapshot.parentId,
        size: snapshot.size,
        pixelSize: undefined,
        data: existing?.data ?? { component: 'GraphEditor' },
      };
      continue;
    }

    next[snapshot.id] = {
      id: snapshot.id,
      type: snapshot.type,
      parentId: snapshot.parentId,
      children: snapshot.children ? [...snapshot.children] : undefined,
      size: snapshot.size,
      pixelSize: undefined,
      data: existing?.data,
    };
  }

  return next;
}

/** Ensure editor_area and default_editor exist after memento apply. */
export function repairEditorGridIntegrity(nodes: LayoutTree): LayoutTree {
  const next = { ...nodes };
  const initial = createInitialWorkbenchNodes();
  const editorArea = next[EDITOR_AREA_ID] ?? initial[EDITOR_AREA_ID];
  next[EDITOR_AREA_ID] = { ...editorArea };

  const children = [...(next[EDITOR_AREA_ID].children ?? [DEFAULT_EDITOR_GROUP_ID])];
  if (!children.includes(DEFAULT_EDITOR_GROUP_ID)) {
    children.unshift(DEFAULT_EDITOR_GROUP_ID);
  }
  next[EDITOR_AREA_ID].children = children;

  if (!next[DEFAULT_EDITOR_GROUP_ID]) {
    next[DEFAULT_EDITOR_GROUP_ID] = {
      ...initial[DEFAULT_EDITOR_GROUP_ID],
      data: { component: 'GraphEditor' },
    };
  }

  for (const childId of next[EDITOR_AREA_ID].children ?? []) {
    if (!next[childId]) {
      if (childId === DEFAULT_EDITOR_GROUP_ID) {
        next[DEFAULT_EDITOR_GROUP_ID] = {
          ...initial[DEFAULT_EDITOR_GROUP_ID],
          data: { component: 'GraphEditor' },
        };
      }
    }
  }

  return next;
}

export function applyEditorGridMementoWithRepair(
  nodes: LayoutTree,
  memento: EditorGridMemento,
): LayoutTree {
  const applied = applyEditorGridMemento(nodes, memento);
  const editorArea = applied[EDITOR_AREA_ID];
  if (editorArea) {
    if (memento.maximizedGroupId) {
      editorArea.data = {
        ...editorArea.data,
        maximizedGroupId: memento.maximizedGroupId,
        restoredGridSizes: memento.restoredGridSizes,
      };
      for (const id of listEditorGroupIds(applied)) {
        setEditorGroupMaximizedHidden(applied, id, id !== memento.maximizedGroupId);
      }
    }
  }
  return repairEditorGridIntegrity(applied);
}