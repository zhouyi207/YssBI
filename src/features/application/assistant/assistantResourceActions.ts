import { resourceKey } from "@/features/core/resource";
import { getResourceSnapshot } from "@/features/core/resource/read";
import { openFileInEditor } from "@/features/application/editor/openFileInEditor";
import { openDatabaseInEditor } from "@/features/application/editor/openDatabaseInEditor";
import {
  RESOURCE_KINDS,
  type ResourceRef,
  type ResourceKind,
} from "@/shared/types/domain/resource";

export function assistantLinkResource(href: string): ResourceRef | null {
  if (href.startsWith("yssbi://")) {
    const match = /^yssbi:\/\/([^/]+)\/(.+)$/.exec(href);
    if (!match || !RESOURCE_KINDS.includes(match[1] as ResourceKind)) return null;
    try {
      return { kind: match[1] as ResourceKind, id: decodeURIComponent(match[2]) };
    } catch {
      return null;
    }
  }
  if (/^[a-z][a-z\d+.-]*:/i.test(href) || href.startsWith("#")) return null;
  let path: string;
  try {
    path = decodeURIComponent(href.split("#")[0]).replace(/^\.\//, "");
  } catch {
    return null;
  }
  const resource = Object.values(getResourceSnapshot().resources).find(
    (resource) => resource.id === path,
  );
  return resource ? { kind: resource.kind, id: resource.id } : null;
}

export async function openAssistantResource(resource: ResourceRef): Promise<void> {
  if (!getResourceSnapshot().resources[resourceKey(resource)]?.exists)
    throw new Error("resource_unavailable");
  if (resource.kind === "database") await openDatabaseInEditor(resource.id);
  else await openFileInEditor(resource.id, resource.kind);
}
