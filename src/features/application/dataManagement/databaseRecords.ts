import { produce } from "immer";
import { updateDatabaseColumns } from "@/features/core/database/databaseProjection";
import {
  resourceKey,
  type ProjectResourceMeta,
  type ResourceKey,
} from "@/features/core/resource/resourceTypes";
import type { ProjectDatabaseIndexRow } from "@/shared/types/domain/project";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { ProjectDatabaseMetadata } from "@/services/database/databaseWireParser";

export function prepareDatabaseIndexSnapshot(
  rows: readonly ProjectDatabaseIndexRow[],
  current: Record<string, DatabaseRecord>,
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  freshMetadata?: Readonly<Record<string, ProjectDatabaseMetadata>>,
): Record<string, DatabaseRecord> {
  if (freshMetadata && Object.keys(freshMetadata).length !== rows.length) {
    throw new TypeError("Project database metadata membership does not match the index");
  }
  return produce(current, (databases) => {
    const ids = new Set<string>();
    for (const row of rows) {
      ids.add(row.id);
      const resource = resources[resourceKey({ kind: "database", id: row.id })];
      const retained =
        resource?.exists && resource.revision === row.revision ? current[row.id] : undefined;
      if (freshMetadata && !Object.prototype.hasOwnProperty.call(freshMetadata, row.id)) {
        throw new TypeError("Project database metadata membership does not match the index");
      }
      const fresh = freshMetadata?.[row.id];
      const metadata = fresh ? { ...retained, ...fresh } : retained;
      const record = (databases[row.id] ??= { id: row.id, name: row.name ?? row.id });
      record.id = row.id;
      record.resourcePath = row.resourcePath;
      record.name = row.name ?? row.id;
      // The validated engine contract is a dataset with no configurable fields.
      record.engine ??= structuredClone(row.engine);
      record.schemaVersion = row.schemaVersion;
      record.required = row.required;
      record.loadFailed = metadata?.loadFailed === true;
      updateDatabaseColumns(record, metadata?.columns);
      if (metadata?.rowCount === undefined) delete record.rowCount;
      else record.rowCount = metadata.rowCount;
      if (metadata?.columnCount === undefined) delete record.columnCount;
      else record.columnCount = metadata.columnCount;
    }
    for (const id of Object.keys(databases)) {
      if (!ids.has(id)) delete databases[id];
    }
  });
}
