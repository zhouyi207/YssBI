import { invokeCommand } from "@/services/ipc";
import { parseGraphEditorSessionDto } from "@/shared/types/dto/editorMutationWireParser";
import type { GraphEditorSessionDto } from "@/shared/types/domain/editorMutation";

export interface GraphSyncBinding {
  projectInstanceId: string;
  graphPath: string;
  locale: string;
}
interface Baseline {
  cursor: string;
  data: GraphEditorSessionDto;
}
export interface GraphSyncReply {
  data: GraphEditorSessionDto;
  changed: boolean;
  resourceRevision: number | null;
  functionEditorProjection: unknown;
}
const baselines = new Map<string, Baseline>();
let epoch = 0;
const key = (binding: GraphSyncBinding) =>
  JSON.stringify([binding.projectInstanceId, binding.graphPath, binding.locale]);
const record = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);
const own = (value: object, key: string) => Object.prototype.hasOwnProperty.call(value, key);
const exact = (value: Record<string, unknown>, keys: string[]) =>
  Object.keys(value).length === keys.length && keys.every((key) => own(value, key));

function applyChanges(previous: GraphEditorSessionDto, changes: unknown): unknown {
  if (!Array.isArray(changes) || changes.length > 512)
    throw new Error("Invalid graph projection changes");
  const copies = new WeakMap<object, Record<string, unknown> | unknown[]>();
  function copy(value: unknown): Record<string, unknown> | unknown[] {
    if (!record(value) && !Array.isArray(value))
      throw new Error("Graph patch parent is not a container");
    const retained = copies.get(value);
    if (retained) return retained;
    const cloned = Array.isArray(value) ? [...value] : { ...value };
    copies.set(value, cloned);
    copies.set(cloned, cloned);
    return cloned;
  }
  function property(container: Record<string, unknown> | unknown[], part: string): string {
    if (
      Array.isArray(container) &&
      (!/^(0|[1-9][0-9]*)$/u.test(part) || Number(part) >= container.length)
    )
      throw new Error("Invalid graph array index");
    return part;
  }
  let candidate: unknown = previous;
  for (const operation of changes) {
    if (
      !record(operation) ||
      (operation.kind !== "set" && operation.kind !== "remove") ||
      !exact(operation, operation.kind === "set" ? ["kind", "path", "value"] : ["kind", "path"]) ||
      !Array.isArray(operation.path) ||
      operation.path.length > 32 ||
      !operation.path.every((part) => typeof part === "string")
    )
      throw new Error("Invalid graph projection operation");
    const path = operation.path as string[];
    if (!path.length) {
      if (operation.kind !== "set") throw new Error("Cannot remove the graph projection root");
      candidate = operation.value;
      continue;
    }
    const root = copy(candidate);
    candidate = root;
    let parent = root;
    for (const segment of path.slice(0, -1)) {
      const part = property(parent, segment);
      if (!own(parent, part)) throw new Error("Missing graph patch parent");
      const child = copy((parent as Record<string, unknown>)[part]);
      Object.defineProperty(parent, part, {
        value: child,
        writable: true,
        configurable: true,
        enumerable: true,
      });
      parent = child;
    }
    const part = property(parent, path[path.length - 1]);
    if (operation.kind === "remove") {
      if (Array.isArray(parent) || !own(parent, part)) throw new Error("Invalid graph removal");
      delete parent[part];
    } else
      Object.defineProperty(parent, part, {
        value: operation.value,
        writable: true,
        configurable: true,
        enumerable: true,
      });
  }
  return candidate;
}

function envelope(value: unknown, binding: GraphSyncBinding): Record<string, unknown> {
  if (
    !record(value) ||
    !exact(value, [
      "projectInstanceId",
      "graphPath",
      "locale",
      "changed",
      "resourceRevision",
      "functionEditorProjection",
      "update",
    ]) ||
    value.projectInstanceId !== binding.projectInstanceId ||
    value.graphPath !== binding.graphPath ||
    value.locale !== binding.locale ||
    typeof value.changed !== "boolean" ||
    (value.resourceRevision !== null &&
      (!Number.isSafeInteger(value.resourceRevision) || (value.resourceRevision as number) < 0))
  ) {
    throw new Error("Graph response binding is malformed");
  }
  return value;
}

function decode(value: Record<string, unknown>, previous: Baseline | undefined): Baseline {
  const update = value.update;
  if (!record(update) || typeof update.cursor !== "string" || !update.cursor)
    throw new Error("Graph delivery cursor is malformed");
  let candidate: unknown;
  if (update.kind === "snapshot" && exact(update, ["kind", "cursor", "data"]))
    candidate = update.data;
  else if (
    update.kind === "delta" &&
    exact(update, ["kind", "baseCursor", "cursor", "changes"]) &&
    previous &&
    update.baseCursor === previous.cursor
  )
    candidate = applyChanges(previous.data, update.changes);
  else throw new Error("Graph delivery baseline is unavailable");
  const data = parseGraphEditorSessionDto(candidate);
  if (data.projection.graphPath !== value.graphPath)
    throw new Error("Graph projection identity mismatch");
  return { cursor: update.cursor, data };
}

type GraphSyncCommand =
  | "load_project_graph"
  | "hydrate_editor_graph"
  | "resolve_editor_graph"
  | "edit_graph"
  | "save_project_graph"
  | "change_graph_history";

function invokeBound(
  command: GraphSyncCommand,
  args: Record<string, unknown>,
  binding: GraphSyncBinding,
  cursor: string | null,
): Promise<unknown> {
  const { projectInstanceId, graphPath, locale } = binding;
  switch (command) {
    case "load_project_graph":
      return invokeCommand("load_project_graph", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
    case "hydrate_editor_graph":
      return invokeCommand("hydrate_editor_graph", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
    case "resolve_editor_graph":
      return invokeCommand("resolve_editor_graph", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
    case "edit_graph":
      return invokeCommand("edit_graph", { ...args, projectInstanceId, graphPath, locale, cursor });
    case "save_project_graph":
      return invokeCommand("save_project_graph", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
    case "change_graph_history":
      return invokeCommand("change_graph_history", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
  }
}

export async function invokeGraphSync(
  command: GraphSyncCommand,
  args: Record<string, unknown>,
  binding: GraphSyncBinding,
): Promise<GraphSyncReply> {
  const requestEpoch = epoch;
  const bindingKey = key(binding);
  const previous = baselines.get(bindingKey);
  const response = envelope(
    await invokeBound(command, args, binding, previous?.cursor ?? null),
    binding,
  );
  let decoded: Baseline;
  try {
    decoded = decode(response, previous);
  } catch {
    // Recover only the read projection. A mutation whose reply was lost must never be replayed.
    const recovered = envelope(
      await invokeBound("hydrate_editor_graph", {}, binding, null),
      binding,
    );
    decoded = decode(recovered, undefined);
  }
  if (requestEpoch !== epoch) throw new Error("Graph response belongs to a released view");
  baselines.delete(bindingKey);
  baselines.set(bindingKey, decoded);
  while (baselines.size > 32) baselines.delete(baselines.keys().next().value!);
  return {
    data: decoded.data,
    changed: response.changed as boolean,
    resourceRevision: response.resourceRevision as number | null,
    functionEditorProjection: response.functionEditorProjection,
  };
}

export function clearGraphSyncBaselines(): void {
  epoch++;
  baselines.clear();
}
