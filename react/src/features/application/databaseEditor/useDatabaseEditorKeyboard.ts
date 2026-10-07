import { useEffect, useRef, type RefObject } from "react";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { isAppModalOpen } from "@/features/core/keyboard";

interface useDatabaseEditorKeyboardParams {
  containerRef: RefObject<HTMLElement | null>;
  selectAll: () => void;
  clearSelection: () => void;
}

export function isTextEntryTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  );
}

export function isSelectAllShortcut(event: {
  ctrlKey: boolean;
  metaKey: boolean;
  key: string;
}): boolean {
  return (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a";
}

export function useDatabaseEditorKeyboard(params: useDatabaseEditorKeyboardParams) {
  const paramsRef = useRef(params);
  paramsRef.current = params;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (
        !(e.target instanceof Node) ||
        !paramsRef.current.containerRef.current?.contains(e.target)
      )
        return;
      if (isAppModalOpen() || isTextEntryTarget(e.target)) {
        return;
      }

      const { selectAll, clearSelection } = paramsRef.current;

      if (e.key === "Escape") {
        clearSelection();
      } else if (isSelectAllShortcut(e)) {
        e.preventDefault();
        selectAll();
      }
    };
    return addGlobalEventListener(window, "keydown", handler);
  }, []);
}
