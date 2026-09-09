import type {
  ActivityPanelDocument,
  ActivityPanelRow,
  ActivityPanelSnapshot,
  BackendActivityPanelId,
} from "../domain/activityPanel";
import { isNodeCreationDescriptorDto } from "../domain/nodeCreationDescriptor";

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function exact(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}
function text(value: unknown): boolean {
  return (
    record(value) &&
    ((exact(value, ["key"]) && typeof value.key === "string") ||
      (exact(value, ["text"]) && typeof value.text === "string"))
  );
}
function integer(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}
const actions = ["install", "refresh"];
function tools(value: unknown): boolean {
  return (
    Array.isArray(value) &&
    value.length <= 8 &&
    new Set(value.map((tool) => (record(tool) ? tool.id : null))).size === value.length &&
    value.every(
      (tool) =>
        record(tool) &&
        exact(tool, ["id", "label", "icon"]) &&
        actions.includes(tool.id as string) &&
        ["add", "install", "refresh"].includes(tool.icon as string) &&
        text(tool.label),
    )
  );
}
function item(value: unknown): boolean {
  if (!record(value)) return false;
  const strings = (keys: string[]) => keys.every((key) => typeof value[key] === "string");
  switch (value.kind) {
    case "node":
      return (
        exact(value, ["kind", "key", "title", "creation"]) &&
        strings(["key", "title"]) &&
        isNodeCreationDescriptorDto(value.creation)
      );
    case "command":
      return (
        exact(value, ["kind", "id", "label"]) &&
        ["undo", "redo"].includes(value.id as string) &&
        text(value.label)
      );
    case "plugin":
      return (
        exact(value, ["kind", "id", "name", "description", "publisher", "enabled"]) &&
        strings(["id", "name", "description", "publisher"]) &&
        typeof value.enabled === "boolean"
      );
    default:
      return false;
  }
}

export function parseActivityPanelDocument(value: unknown): ActivityPanelDocument | null {
  if (
    !record(value) ||
    !exact(value, [
      "schema",
      "panelId",
      "projectInstanceId",
      "publicationRevision",
      "title",
      "tools",
      "rows",
      "emptyState",
    ])
  )
    return null;
  if (
    value.emptyState !== null &&
    (!record(value.emptyState) ||
      !exact(value.emptyState, ["title", "description"]) ||
      !text(value.emptyState.title) ||
      !text(value.emptyState.description))
  )
    return null;
  if (
    value.schema !== "yssbi.activity-panel.v1" ||
    !["nodes", "commands", "plugins"].includes(value.panelId as string)
  )
    return null;
  if (value.projectInstanceId !== null && typeof value.projectInstanceId !== "string") return null;
  if (!integer(value.publicationRevision) || !text(value.title) || !tools(value.tools)) return null;
  if (!Array.isArray(value.rows) || value.rows.length > 10_000) return null;
  const ids = new Set<string>();
  let previousDepth = -1;
  let previousCategory = true;
  for (const row of value.rows) {
    if (
      !record(row) ||
      typeof row.id !== "string" ||
      !row.id ||
      ids.has(row.id) ||
      !integer(row.depth) ||
      row.depth > 32
    )
      return null;
    if (row.depth > previousDepth && (row.depth !== previousDepth + 1 || !previousCategory))
      return null;
    previousDepth = row.depth;
    previousCategory = row.kind === "category";
    ids.add(row.id);
    switch (row.kind) {
      case "category":
        if (
          !exact(row, ["id", "depth", "kind", "label", "defaultExpanded", "tools", "count"]) ||
          !text(row.label) ||
          typeof row.defaultExpanded !== "boolean" ||
          !tools(row.tools) ||
          !(row.count === null || integer(row.count))
        )
          return null;
        break;
      case "item":
        if (!exact(row, ["id", "depth", "kind", "item"]) || !item(row.item)) return null;
        if (
          !record(row.item) ||
          !{
            nodes: ["node"],
            commands: ["command"],
            plugins: ["plugin"],
          }[value.panelId as BackendActivityPanelId].includes(row.item.kind as string)
        )
          return null;
        break;
      case "message":
        if (
          !exact(row, ["id", "depth", "kind", "label", "description"]) ||
          !text(row.label) ||
          !(row.description === null || text(row.description))
        )
          return null;
        break;
      default:
        return null;
    }
  }
  return value as unknown as ActivityPanelDocument;
}

function cursor(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= 64;
}

const HEADER_PATCH_FIELDS = new Set(["title", "tools", "emptyState", "publicationRevision"]);
const ROW_PATCH_FIELDS = new Set([
  "depth",
  "label",
  "defaultExpanded",
  "tools",
  "count",
  "item",
  "description",
]);

/** Apply wire operations to an isolated copy; a rejected batch never mutates the visible projection. */
export function parseActivityPanelUpdate(
  value: unknown,
  previous: ActivityPanelSnapshot | null,
): ActivityPanelSnapshot | null {
  if (!record(value) || !cursor(value.cursor)) return null;
  if (value.kind === "snapshot") {
    if (!exact(value, ["kind", "cursor", "document"])) return null;
    const document = parseActivityPanelDocument(value.document);
    return document ? { cursor: value.cursor, document } : null;
  }
  if (
    value.kind !== "patch" ||
    !exact(value, ["kind", "baseCursor", "cursor", "patch", "operations"]) ||
    !previous ||
    value.baseCursor !== previous.cursor
  )
    return null;
  if (!record(value.patch) || Object.keys(value.patch).some((key) => !HEADER_PATCH_FIELDS.has(key)))
    return null;
  if (!Array.isArray(value.operations) || value.operations.length > 30_000) return null;
  if (value.operations.length === 0 && Object.keys(value.patch).length === 0) {
    return value.cursor === previous.cursor
      ? previous
      : { cursor: value.cursor, document: previous.document };
  }
  if (value.cursor === previous.cursor) return null;

  const rows: unknown[] = value.operations.length ? [...previous.document.rows] : [];
  let indices: Map<string, number> | null = null;
  const find = (id: string) => {
    indices ??= new Map(rows.map((row, index) => [(row as ActivityPanelRow).id, index]));
    return indices.get(id) ?? -1;
  };
  for (const operation of value.operations) {
    if (!record(operation)) return null;
    if (operation.op === "insert") {
      if (
        !exact(operation, ["op", "afterId", "row"]) ||
        !record(operation.row) ||
        typeof operation.row.id !== "string" ||
        !operation.row.id ||
        find(operation.row.id) !== -1
      )
        return null;
      if (operation.afterId !== null && typeof operation.afterId !== "string") return null;
      const after = operation.afterId === null ? -1 : find(operation.afterId);
      if (operation.afterId !== null && after < 0) return null;
      rows.splice(after + 1, 0, operation.row);
      indices = null;
      continue;
    }
    if (typeof operation.id !== "string") return null;
    const index = find(operation.id);
    if (index < 0) return null;
    switch (operation.op) {
      case "update":
        if (
          !exact(operation, ["op", "id", "patch"]) ||
          !record(operation.patch) ||
          Object.keys(operation.patch).length === 0 ||
          Object.keys(operation.patch).some((key) => !ROW_PATCH_FIELDS.has(key))
        )
          return null;
        rows[index] = { ...(rows[index] as ActivityPanelRow), ...operation.patch };
        break;
      case "remove":
        if (!exact(operation, ["op", "id"])) return null;
        rows.splice(index, 1);
        indices = null;
        break;
      case "move": {
        if (
          !exact(operation, ["op", "id", "afterId"]) ||
          operation.afterId === operation.id ||
          (operation.afterId !== null && typeof operation.afterId !== "string")
        )
          return null;
        const [row] = rows.splice(index, 1);
        indices = null;
        const after = operation.afterId === null ? -1 : find(operation.afterId);
        if (operation.afterId !== null && after < 0) return null;
        rows.splice(after + 1, 0, row);
        indices = null;
        break;
      }
      default:
        return null;
    }
  }
  const document = parseActivityPanelDocument({
    ...previous.document,
    ...value.patch,
    rows: value.operations.length ? rows : previous.document.rows,
  });
  if (!document || document.publicationRevision < previous.document.publicationRevision)
    return null;
  return { cursor: value.cursor, document };
}
