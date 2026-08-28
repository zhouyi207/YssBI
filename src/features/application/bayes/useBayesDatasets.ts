import { useEffect, useMemo, useRef, useState } from 'react';
import { databaseRead, useDatabaseRead } from '@/features/core/database/read';
import { createDatabasePublication } from '@/features/core/database/publication';
import { useProjectIOStore } from '@/features/application/project/projectIOStore';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { DatabaseService } from '@/services/database/databaseService';
import { toErrorReference, type ErrorReference } from '@/services/ipc';
import { databaseRecordFromLoad, type DatabaseRecord } from '@/shared/types/dto/database';
import type {
  BayesColumnDTypeDTO,
  BayesColumnMetaDTO,
  BayesDatasetSelectionDTO,
} from '@/shared/types/bayes';
import type { DeepReadonly } from '@/features/core/projection/deepReadonly';

export interface BayesDatasetOption {
  readonly sourceType: BayesDatasetSelectionDTO['sourceType'];
  readonly sourceId: string;
  readonly columns: readonly BayesColumnMetaDTO[];
  readonly displayName: string;
}

export interface BayesDatasetsModel {
  readonly datasets: DeepReadonly<readonly BayesDatasetOption[]>;
  readonly loading: boolean;
  readonly issue: ErrorReference | null;
}

interface DatasetRequest {
  readonly generation: number;
  readonly identity: ProjectIdentitySnapshot;
  readonly revisions: Readonly<Record<string, number>>;
}

export function useBayesDatasets(): BayesDatasetsModel {
  const snapshot = useDatabaseRead((current) => current);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const [loading, setLoading] = useState(false);
  const [issue, setIssue] = useState<ErrorReference | null>(null);
  const requestGeneration = useRef(0);
  const publication = useMemo(() => createDatabasePublication(), []);

  const datasets = useMemo(
    () => Object.values(snapshot.databases).map(toBayesDataset),
    [snapshot.databases],
  );

  useEffect(() => {
    const missingMetadata = Object.values(snapshot.databases)
      .filter((database) => (database.columns?.length ?? 0) === 0);
    const generation = ++requestGeneration.current;
    if (missingMetadata.length === 0) {
      setLoading(false);
      setIssue(null);
      return;
    }

    let identity: ProjectIdentitySnapshot;
    try {
      identity = captureProjectIdentity();
    } catch {
      setLoading(false);
      setIssue(null);
      return;
    }

    const request: DatasetRequest = {
      generation,
      identity,
      revisions: snapshot.revisions,
    };
    setLoading(true);
    setIssue(null);

    void Promise.all(missingMetadata.map(async (database) => {
      try {
        return {
          database,
          meta: await DatabaseService.getDatabaseMeta(identity.projectInstanceId, database.id),
          error: null,
        };
      } catch (error) {
        return { database, meta: null, error };
      }
    })).then((results) => {
      if (!isCurrentRequest(request, requestGeneration.current)) return;

      const current = databaseRead.getSnapshot();
      let nextIssue: ErrorReference | null = null;
      for (const result of results) {
        if (result.error) {
          nextIssue ??= toErrorReference(result.error, 'bayes_dataset_metadata_read_failed');
          continue;
        }
        if (!result.meta || !isCurrentDatabaseRevision(current.revisions, request.revisions, result.database.id)) {
          continue;
        }
        publication.publishDatabase(
          databaseRecordFromLoad(result.meta, current.databases[result.database.id] as DatabaseRecord | undefined),
        );
      }
      setIssue(nextIssue);
      setLoading(false);
    });

    return () => {
      if (requestGeneration.current === generation) requestGeneration.current += 1;
    };
  }, [projectInstanceId, publication, snapshot.databases, snapshot.revisions]);

  return { datasets, loading, issue };
}

function isCurrentRequest(request: DatasetRequest, currentGeneration: number): boolean {
  return request.generation === currentGeneration && isCurrentProjectIdentity(request.identity);
}

function isCurrentDatabaseRevision(
  current: Readonly<Record<string, number>>,
  requested: Readonly<Record<string, number>>,
  databaseId: string,
): boolean {
  return current[databaseId] === requested[databaseId];
}

function toBayesDataset(database: DeepReadonly<DatabaseRecord>): BayesDatasetOption {
  return {
    sourceType: 'table',
    sourceId: database.id,
    displayName: database.name,
    columns: (database.columns ?? []).map((column) => ({
      name: column.name,
      dtype: bayesColumnDType(column.type),
      nullable: true,
    })),
  };
}

export function bayesColumnDType(type: string): BayesColumnDTypeDTO {
  const normalized = type.toLowerCase();
  if (normalized.includes('int')) return 'integer';
  if (
    normalized.includes('float')
    || normalized.includes('double')
    || normalized.includes('real')
    || normalized.includes('decimal')
    || normalized.includes('numeric')
  ) return 'number';
  if (normalized.includes('bool')) return 'boolean';
  if (normalized.includes('date') || normalized.includes('time')) return 'date';
  if (normalized.includes('char') || normalized.includes('text') || normalized.includes('string')) return 'string';
  return 'unknown';
}
