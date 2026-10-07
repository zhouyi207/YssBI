import type { EditorViewport } from "./editorViewport";
import { logger } from "@/utils/frontendLogger";
import { normalizeEditorViewport } from "./editorViewport";
import { z } from "zod";

const STORAGE_PREFIX = "yssbi-editor-view-state";

export type EditorViewStateMemento = Record<string, EditorViewport>;

const editorViewStateSchema = z.record(
  z.string(),
  z
    .object({
      x: z.number().finite(),
      y: z.number().finite(),
      scale: z.number().finite().positive(),
    })
    .strict(),
);

export function editorViewStateStorageKey(projectPath: string): string {
  return `${STORAGE_PREFIX}:${encodeURIComponent(projectPath)}`;
}

export function loadEditorViewStateMemento(projectPath: string): EditorViewStateMemento {
  if (typeof localStorage === "undefined") return {};

  try {
    const raw = localStorage.getItem(editorViewStateStorageKey(projectPath));
    if (!raw) return {};
    const parsed = editorViewStateSchema.safeParse(JSON.parse(raw));
    return parsed.success ? parsed.data : {};
  } catch (error) {
    logger.app.warn(
      `Failed to load editor view state: ${error instanceof Error ? error.message : String(error)}`,
      "EditorViewState",
    );
    return {};
  }
}

export function saveEditorViewStateMemento(
  projectPath: string,
  memento: EditorViewStateMemento,
): void {
  if (typeof localStorage === "undefined") return;

  try {
    localStorage.setItem(editorViewStateStorageKey(projectPath), JSON.stringify(memento));
  } catch (error) {
    logger.app.warn(
      `Failed to save editor view state: ${error instanceof Error ? error.message : String(error)}`,
      "EditorViewState",
    );
  }
}

export function readEditorViewStateViewport(
  projectPath: string,
  graphPath: string,
): EditorViewport | null {
  const viewport = loadEditorViewStateMemento(projectPath)[graphPath];
  return viewport ? normalizeEditorViewport(viewport) : null;
}

export function patchEditorViewStateViewport(
  projectPath: string,
  graphPath: string,
  viewport: EditorViewport,
): void {
  const normalized = normalizeEditorViewport(viewport);
  const memento = loadEditorViewStateMemento(projectPath);
  const prev = memento[graphPath];
  if (
    prev &&
    prev.x === normalized.x &&
    prev.y === normalized.y &&
    prev.scale === normalized.scale
  ) {
    return;
  }
  saveEditorViewStateMemento(projectPath, { ...memento, [graphPath]: normalized });
}

export function remapEditorViewStateGraphPath(projectPath: string, from: string, to: string): void {
  if (from === to) return;
  const memento = loadEditorViewStateMemento(projectPath);
  const viewport = memento[from];
  if (!viewport) return;
  const next = { ...memento };
  delete next[from];
  next[to] = viewport;
  saveEditorViewStateMemento(projectPath, next);
}
