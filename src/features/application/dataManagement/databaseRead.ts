import { useEffect } from "react";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useResourceRead } from "@/features/core/resource/read";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { DatabaseService } from "@/services/database/databaseService";
import { logger } from "@/utils/frontendLogger";
import { formatApplicationIpcError } from "@/features/application/errorReference";

export interface DatabaseRead {
  readonly id: string;
  readonly projectInstanceId: string;
  readonly revision: number | undefined;
  isCurrent(): boolean;
}

export function captureDatabaseRead(
  identity: ProjectIdentitySnapshot,
  id: string,
  isActive: () => boolean = () => true,
): DatabaseRead {
  const key = resourceKey({ kind: "database", id });
  const revision = useResourceStore.getState().resources[key]?.revision;
  return {
    id,
    projectInstanceId: identity.projectInstanceId,
    revision,
    isCurrent: () => {
      if (!isActive() || !isCurrentProjectIdentity(identity) || revision === undefined)
        return false;
      const state = useResourceStore.getState();
      const resource = state.resources[key];
      return resource?.exists === true && resource.revision === revision && !!state.databases[id];
    },
  };
}

export async function readDatabaseMetadata(read: DatabaseRead): Promise<void> {
  if (read.revision === undefined || !read.isCurrent()) return;
  const metadata = await DatabaseService.getDatabaseMeta(
    read.projectInstanceId,
    read.id,
    read.revision,
  );
  if (read.isCurrent())
    useResourceStore.getState().updateDatabaseMetadata(read.id, read.revision, metadata);
}

export async function readDatabasePage(read: DatabaseRead, pageIndex: number, pageSize: number) {
  if (read.revision === undefined || !read.isCurrent()) return null;
  const rowCount = useResourceStore.getState().databases[read.id]?.rowCount;
  const lastPage =
    rowCount === undefined ? pageIndex : Math.max(0, Math.ceil(rowCount / pageSize) - 1);
  const safePageIndex = Math.max(0, Math.min(pageIndex, lastPage));
  const page = await DatabaseService.getDatabaseRows(
    read.projectInstanceId,
    read.id,
    read.revision,
    safePageIndex * pageSize,
    pageSize,
  );
  if (!read.isCurrent()) return null;
  const columnCount = useResourceStore.getState().databases[read.id]?.columnCount;
  if (columnCount !== undefined && page.rows.length && page.rows[0].length !== columnCount) {
    throw new TypeError("Database page width does not match its metadata");
  }
  return { ...page, pageIndex: safePageIndex };
}

export async function hydrateDatabaseEditorMetadata(
  id: string,
  isCancelled: () => boolean = () => false,
): Promise<void> {
  const read = captureDatabaseRead(captureProjectIdentity(), id, () => !isCancelled());
  try {
    await readDatabaseMetadata(read);
  } catch (error) {
    if (read.isCurrent()) {
      logger.data.warn(
        "getDatabaseMeta failed: " + formatApplicationIpcError(error),
        "DatabaseEditorWindow",
      );
    }
  }
}

export function useDatabaseMetadata(
  id: string | undefined,
  metadataReady: boolean,
): number | undefined {
  const revision = useResourceRead((snapshot) =>
    id ? snapshot.resources[resourceKey({ kind: "database", id })]?.revision : undefined,
  );
  useEffect(() => {
    if (!id || revision === undefined || metadataReady) return;
    let cancelled = false;
    void hydrateDatabaseEditorMetadata(id, () => cancelled);
    return () => {
      cancelled = true;
    };
  }, [id, revision, metadataReady]);
  return revision;
}
