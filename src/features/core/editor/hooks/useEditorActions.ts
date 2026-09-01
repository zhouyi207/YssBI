/**
 * 编辑器操作（组合 hook）
 * 组合 useEditorCanvasActions、useEditorUIActions
 * 并提供 refs 供 canvas pointer loop 使用（viewportRef 为 EditorViewport 快照）
 */
import { useCallback, useEffect, useRef } from "react";
import { DEFAULT_VIEWPORT } from "@/shared/config-default";
import {
  commitViewport,
  editorViewportScope,
  getViewport,
  setViewportLive,
  subscribeToViewport,
  type EditorViewport,
} from "@/features/core/viewport";
import { useActiveEditorGroup } from "./useActiveEditorGroup";
import { useEditorUIActions } from "./useEditorUIActions";

type ActiveEditorGroup = ReturnType<typeof useActiveEditorGroup>;

export function useEditorActions(active: ActiveEditorGroup) {
  const editorGroupId = active.groupId;
  const activeGroupIdRef = useRef<string | null>(editorGroupId);
  const activeResourceRefRef = useRef<string | null>(active.activeResourceRef);
  activeGroupIdRef.current = editorGroupId;
  activeResourceRefRef.current = active.activeResourceRef;

  const setCanvas = useCallback(
    (
      updater: EditorViewport | ((previous: EditorViewport) => EditorViewport),
      targetGraphPath?: string,
    ) => {
      const groupId = activeGroupIdRef.current;
      const graphPath = targetGraphPath ?? activeResourceRefRef.current;
      if (!groupId || !graphPath) return;
      const scope = editorViewportScope(groupId, graphPath);
      setViewportLive(scope, updater);
      commitViewport(scope);
    },
    [],
  );
  const uiActions = useEditorUIActions();

  const viewportScope =
    editorGroupId && active.activeResourceRef
      ? editorViewportScope(editorGroupId, active.activeResourceRef)
      : null;

  const viewportRef = useRef(viewportScope ? getViewport(viewportScope) : DEFAULT_VIEWPORT);

  useEffect(() => {
    if (!viewportScope) return;
    viewportRef.current = getViewport(viewportScope);
    return subscribeToViewport(viewportScope, (viewport) => {
      viewportRef.current = viewport;
    });
  }, [viewportScope?.groupId, viewportScope?.graphPath]);

  return {
    activeGroupIdRef,
    activeResourceRefRef,
    viewportRef,
    setCanvas,
    ...uiActions,
  };
}
