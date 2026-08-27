import { toErrorReference, type ErrorReference } from '@/services/ipc';
import type { DatabaseRecord } from '@/shared/types/dto/database';
import type { DatabaseId } from '@/shared/types/domain/ids';
import type { ProjectIdentitySnapshot } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import type { DatabasePublicationCapability } from '@/features/core/database/publication';

export interface DatabaseMetadataReader {
  readonly read: (
    projectInstanceId: string,
    databaseId: DatabaseId,
  ) => Promise<DatabaseRecord | null>;
}

export interface DatabaseMetadataProjectIdentity {
  readonly capture: () => ProjectIdentitySnapshot | null;
  readonly isCurrent: (identity: ProjectIdentitySnapshot) => boolean;
}

export interface DatabaseMetadataCoordinatorDependencies {
  readonly project: DatabaseMetadataProjectIdentity;
  readonly reader: DatabaseMetadataReader;
  readonly publication: Pick<DatabasePublicationCapability, 'publishDatabase' | 'publishDatabaseFailure'>;
  readonly toErrorReference?: (error: unknown, operation: string) => ErrorReference;
}

export type DatabaseMetadataOutcome =
  | { readonly status: 'published' }
  | { readonly status: 'stale' }
  | { readonly status: 'notReady' }
  | { readonly status: 'failed' };

interface RequestGeneration {
  readonly projectGeneration: number;
  readonly databaseGeneration: number;
  readonly identity: ProjectIdentitySnapshot;
}

export class DatabaseMetadataCoordinator {
  private projectGeneration = 0;
  private readonly databaseGenerations = new Map<DatabaseId, number>();

  constructor(private readonly dependencies: DatabaseMetadataCoordinatorDependencies) {}

  async load(databaseId: DatabaseId): Promise<DatabaseMetadataOutcome> {
    if (!databaseId.trim()) return { status: 'notReady' };
    const identity = this.dependencies.project.capture();
    if (!identity) return { status: 'notReady' };

    const request: RequestGeneration = {
      projectGeneration: this.projectGeneration,
      databaseGeneration: this.nextDatabaseGeneration(databaseId),
      identity,
    };

    let database: DatabaseRecord | null;
    try {
      database = await this.dependencies.reader.read(identity.projectInstanceId, databaseId);
    } catch (error) {
      if (!this.isCurrent(databaseId, request)) return { status: 'stale' };
      const toError = this.dependencies.toErrorReference ?? toErrorReference;
      this.dependencies.publication.publishDatabaseFailure(
        databaseId,
        toError(error, 'database_metadata_read_failed'),
      );
      return { status: 'failed' };
    }

    if (!this.isCurrent(databaseId, request)) return { status: 'stale' };
    if (!database || database.id !== databaseId) return { status: 'notReady' };

    this.dependencies.publication.publishDatabase(database);
    return { status: 'published' };
  }

  resetProject(): void {
    this.projectGeneration += 1;
    this.databaseGenerations.clear();
  }

  resetDatabase(databaseId: DatabaseId): void {
    this.nextDatabaseGeneration(databaseId);
  }

  private nextDatabaseGeneration(databaseId: DatabaseId): number {
    const generation = (this.databaseGenerations.get(databaseId) ?? 0) + 1;
    this.databaseGenerations.set(databaseId, generation);
    return generation;
  }

  private isCurrent(databaseId: DatabaseId, request: RequestGeneration): boolean {
    return this.projectGeneration === request.projectGeneration
      && this.databaseGenerations.get(databaseId) === request.databaseGeneration
      && this.dependencies.project.isCurrent(request.identity);
  }
}
